//! Staged Dealer value reservation and reverse-order expiry rollback.
//!
//! A release-authenticated Trading checkpoint is the durable authorization.
//! Reserve is permissionless: it can only execute the exact evaluator-owned
//! effect already sealed by that checkpoint. Custody moves the source atoms
//! into a checkpoint/effect-scoped `RecoveryReserve` vault and emits a durable
//! typed receipt. Trading ingests that receipt in the same transaction or a
//! later recovery transaction. Rollback is likewise permissionless after the
//! checkpoint expiry and returns escrows in strict reverse order.

use alloc::{boxed::Box, vec};
use core::convert::TryFrom;

use dclutch_custody_contract::{
    CUSTODY_REPLAY_BYTES_V1, CompartmentV1, CustodyReplayV1, CustodyRequestV1, CustodyVaultSeedsV1,
    OperationV1,
};
use dclutch_dealer_codec::{
    scenario_checkpoint_v1::{
        DEALER_SCENARIO_CHECKPOINT_BYTES_V1, DEALER_SCENARIO_CHECKPOINT_PDA_DOMAIN_V1,
        DealerScenarioCheckpointInputV1, DealerScenarioCheckpointPhaseV1,
        DealerScenarioCheckpointV1,
    },
    scenario_custody_reservation_v1::{
        DEALER_SCENARIO_ACTIVATION_RECEIPT_BYTES_V1,
        DEALER_SCENARIO_ACTIVATION_RECEIPT_PDA_DOMAIN_V1, DEALER_SCENARIO_CUSTODY_EFFECT_BYTES_V1,
        DEALER_SCENARIO_CUSTODY_EFFECT_MANIFEST_BYTES_V1,
        DEALER_SCENARIO_RESERVATION_BATCH_BYTES_V1,
        DEALER_SCENARIO_RESERVATION_BATCH_PDA_DOMAIN_V1,
        DEALER_SCENARIO_RESERVATION_STATE_BYTES_V1,
        DEALER_SCENARIO_RESERVATION_STATE_PDA_DOMAIN_V1, DealerScenarioActivationReceiptV1,
        DealerScenarioCustodyEffectManifestV1, DealerScenarioCustodyEffectV1,
        DealerScenarioCustodyRequestKindV1, DealerScenarioReservationBatchStatusV1,
        DealerScenarioReservationBatchV1, DealerScenarioReservationStateStatusV1,
        DealerScenarioReservationStateV1, decode_dealer_scenario_activation_instruction_v1,
        decode_dealer_scenario_reservation_instruction_v1,
    },
    scenario_reservation_receipt_v1::{
        DEALER_SCENARIO_RESERVATION_RECEIPT_BYTES_V1,
        DEALER_SCENARIO_RESERVATION_RECEIPT_PDA_DOMAIN_V1, DealerScenarioReservationActionV1,
        DealerScenarioReservationReceiptV1,
    },
};
use dclutch_release_set_contract::ExecutionRoleV1;
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    hash::hash,
    program::{invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::create_account;

use crate::{
    CustodySbfError, PoststateProjection, TransferAccounts, account, authenticate_calling_release,
    authenticate_market, authenticate_realm, authenticate_replay_identity,
    authenticate_transfer_accounts, create_vault, initialize_vault, invoke_close,
    invoke_exact_transfer, poststate_commitment, read_replay, validate_custody_authority,
    validate_token_program_and_mint, validate_vault_key,
};

/// Exact Reserve/Rollback account count; the outer transaction adds Custody.
pub(crate) const DEALER_SCENARIO_RESERVATION_ACCOUNT_COUNT_V1: usize = 26;

const MARKET: usize = 0;
const CACHE: usize = 1;
const REGISTRY: usize = 2;
const TRADING_PROGRAM: usize = 3;
const TRADING_PROGRAMDATA: usize = 4;
const REALM: usize = 5;
const REALM_STAGING: usize = 6;
const REPLAY: usize = 7;
const CHECKPOINT: usize = 8;
const EFFECT_PRODUCER: usize = 9;
const EFFECT_MANIFEST: usize = 10;
const EFFECT_BODY: usize = 11;
const BATCH: usize = 12;
const RESERVATION_STATE: usize = 13;
const RECEIPT: usize = 14;
const SOURCE: usize = 15;
const DESTINATION: usize = 16;
const ESCROW: usize = 17;
const MINT: usize = 18;
const CUSTODY_AUTHORITY: usize = 19;
const TOKEN_PROGRAM: usize = 20;
const PAYER: usize = 21;
const REFUND: usize = 22;
const CLOCK: usize = 23;
const RENT: usize = 24;
const SYSTEM: usize = 25;

const ACT_COMMON_ACCOUNT_COUNT: usize = 20;
const ACT_EFFECT_ACCOUNT_COUNT: usize = 4;
const ACT_REPLAY: usize = 7;
const ACT_CHECKPOINT: usize = 8;
const ACT_EFFECT_PRODUCER: usize = 9;
const ACT_MANIFEST: usize = 10;
const ACT_BATCH: usize = 11;
const ACT_RECEIPT: usize = 12;
const ACT_MINT: usize = 13;
const ACT_CUSTODY_AUTHORITY: usize = 14;
const ACT_TOKEN_PROGRAM: usize = 15;
const ACT_PAYER: usize = 16;
const ACT_REFUND: usize = 17;
const ACT_RENT: usize = 18;
const ACT_SYSTEM: usize = 19;

struct AuthenticatedEffectV1 {
    input: DealerScenarioCheckpointInputV1,
    checkpoint_reservation_receipt: [u8; 32],
    checkpoint_digest: [u8; 32],
    effect_count: u8,
    source_after: u64,
    destination_after: u64,
    effect_digest: [u8; 32],
    effects_digest: [u8; 32],
    custody: CustodyRequestV1,
    slot: u64,
}

struct CheckpointFactsV1 {
    input: DealerScenarioCheckpointInputV1,
    digest: [u8; 32],
    phase: DealerScenarioCheckpointPhaseV1,
    reservation_count: u8,
    rollback_count: u8,
    reservation_receipt: [u8; 32],
    effect_count: u8,
    effects_digest: [u8; 32],
}

struct ManifestFactsV1 {
    effect_count: u8,
    effects_digest: [u8; 32],
    effect_digest: [u8; 32],
}

struct DecodedEffectV1 {
    effect_count: u8,
    source_after: u64,
    destination_after: u64,
    effect_digest: [u8; 32],
    request_wire_digest: [u8; 32],
    custody: CustodyRequestV1,
}

/// Recognize only this exact instruction family.
pub(crate) fn is_instruction(data: &[u8]) -> bool {
    decode_dealer_scenario_reservation_instruction_v1(data).is_ok()
        || decode_dealer_scenario_activation_instruction_v1(data).is_ok()
}

/// Execute one exact reserve or reverse rollback transition.
#[inline(never)]
pub(crate) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    if let Ok(effect_count) = decode_dealer_scenario_activation_instruction_v1(instruction_data) {
        return activate_batch(program_id, accounts, effect_count);
    }
    let (action, ordinal) = decode_dealer_scenario_reservation_instruction_v1(instruction_data)
        .map_err(|_| CustodySbfError::Instruction)?;
    require_frame(accounts, action)?;
    let authenticated = authenticate_effect(program_id, accounts, action, ordinal)?;
    match action {
        DealerScenarioReservationActionV1::Reserve => {
            reserve(program_id, accounts, ordinal, &authenticated)
        }
        DealerScenarioReservationActionV1::Rollback => {
            rollback(program_id, accounts, ordinal, &authenticated)
        }
    }
}

fn require_frame(
    accounts: &[AccountInfo<'_>],
    action: DealerScenarioReservationActionV1,
) -> Result<(), ProgramError> {
    if accounts.len() != DEALER_SCENARIO_RESERVATION_ACCOUNT_COUNT_V1 {
        return Err(CustodySbfError::AccountFrame.into());
    }
    let writable = [BATCH, RESERVATION_STATE, RECEIPT, SOURCE, ESCROW, PAYER];
    let executable = [
        REGISTRY,
        TRADING_PROGRAM,
        EFFECT_PRODUCER,
        TOKEN_PROGRAM,
        SYSTEM,
    ];
    for (index, current) in accounts.iter().enumerate() {
        let expected_writable = writable.contains(&index)
            || (index == REFUND && action == DealerScenarioReservationActionV1::Rollback);
        let expected_signer = index == PAYER;
        let expected_executable = executable.contains(&index);
        // The checkpoint's WRITABILITY is not this frame's to pin. Custody only
        // reads the checkpoint here -- it authenticates the owner, the PDA and
        // the phase, and writes nothing -- while the supported way to reserve is
        // the operator's atomic producer-then-ingest pair, whose second
        // instruction is Trading's ingest and necessarily takes the checkpoint
        // WRITABLE. Solana merges account privileges across the instructions of
        // one transaction, so a readonly pin here was not a constraint on this
        // instruction at all: it was a constraint on the caller's other
        // instruction, and it made the only shape in which a reservation can be
        // both produced and joined unsubmittable. Identity, owner and phase
        // stay pinned; signer and executable stay pinned.
        let writability_pinned = index != CHECKPOINT;
        if (writability_pinned && current.is_writable != expected_writable)
            || current.is_signer != expected_signer
            || current.executable != expected_executable
        {
            return Err(CustodySbfError::AccountFrame.into());
        }
    }
    // Exactly the contradiction the activation frame already carries, one leg
    // earlier, and it made this route unreachable for every scenario that could
    // ever commit. Trading's evaluate refuses any effect manifest whose
    // `producer_program` is not the Trading program, and its commit refuses one
    // again on the same equality -- so a manifest that can be reserved against
    // AND committed must name Trading as its producer. This frame carries the
    // effect producer at `EFFECT_PRODUCER` and the calling Trading release at
    // `TRADING_PROGRAM`, and then forbade any key from repeating. It was
    // required to repeat one key and forbidden to repeat any key: unsatisfiable
    // by construction, for every input. Pin the equality POSITIVELY and excuse
    // that one slot from the census, as `require_activation_frame` does.
    if account(accounts, EFFECT_PRODUCER)?.key != account(accounts, TRADING_PROGRAM)?.key {
        return Err(CustodySbfError::AccountFrame.into());
    }
    if account(accounts, CLOCK)?.key != &sysvar::clock::ID
        || account(accounts, RENT)?.key != &sysvar::rent::ID
        || account(accounts, SYSTEM)?.key != &system_program::ID
        || account(accounts, REGISTRY)?.key == account(accounts, TRADING_PROGRAM)?.key
        || has_duplicate_keys_except(accounts, EFFECT_PRODUCER)
    {
        return Err(CustodySbfError::AccountFrame.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_effect(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    action: DealerScenarioReservationActionV1,
    ordinal: u8,
) -> Result<Box<AuthenticatedEffectV1>, ProgramError> {
    let trading = account(accounts, TRADING_PROGRAM)?;
    let checkpoint_account = account(accounts, CHECKPOINT)?;
    let checkpoint = read_checkpoint_facts(trading.key, checkpoint_account, ordinal)?;
    let producer = account(accounts, EFFECT_PRODUCER)?;
    let manifest_account = account(accounts, EFFECT_MANIFEST)?;
    let effect_account = account(accounts, EFFECT_BODY)?;
    let manifest = read_manifest_facts(
        producer,
        manifest_account,
        effect_account,
        checkpoint_account.key,
        checkpoint.input.request_digest,
        ordinal,
    )?;
    if manifest.effect_count != checkpoint.effect_count
        || manifest.effects_digest != checkpoint.effects_digest
    {
        return Err(CustodySbfError::Release.into());
    }
    let effect = read_effect_facts(
        producer,
        effect_account,
        checkpoint_account.key,
        checkpoint.input.request_digest,
        ordinal,
        manifest.effect_count,
        manifest.effect_digest,
    )?;
    let custody = effect.custody;
    if custody.operation != OperationV1::Transfer
        || custody.caller_role != ExecutionRoleV1::Trading
        || custody.caller_program != trading.key.to_bytes()
        || custody.release_set != checkpoint.input.release_set
        || custody.market != checkpoint.input.market
        || custody.semantic.parent_request_digest != checkpoint.input.request_digest
        || custody.semantic.generation != checkpoint.input.generation
        || custody.semantic.transfer_index != u16::from(ordinal)
        || custody.source != account(accounts, SOURCE)?.key.to_bytes()
        || custody.destination != account(accounts, DESTINATION)?.key.to_bytes()
        || custody.mint != account(accounts, MINT)?.key.to_bytes()
        || custody.token_program != account(accounts, TOKEN_PROGRAM)?.key.to_bytes()
        || checkpoint.input.refund_beneficiary != account(accounts, REFUND)?.key.to_bytes()
    {
        return Err(CustodySbfError::Release.into());
    }
    let slot = Clock::from_account_info(account(accounts, CLOCK)?)
        .map_err(|_| CustodySbfError::AccountFrame)?
        .slot;
    match action {
        DealerScenarioReservationActionV1::Reserve => {
            if checkpoint.phase != DealerScenarioCheckpointPhaseV1::Evaluated
                || checkpoint.reservation_count != ordinal
                || slot > checkpoint.input.expires_at
            {
                return Err(CustodySbfError::Expiry.into());
            }
        }
        DealerScenarioReservationActionV1::Rollback => {
            let expected = checkpoint
                .reservation_count
                .checked_sub(checkpoint.rollback_count)
                .and_then(|value| value.checked_sub(1))
                .ok_or(CustodySbfError::Replay)?;
            if !matches!(
                checkpoint.phase,
                DealerScenarioCheckpointPhaseV1::Reserved
                    | DealerScenarioCheckpointPhaseV1::RollingBack
            ) || expected != ordinal
                || slot <= checkpoint.input.expires_at
            {
                return Err(CustodySbfError::Expiry.into());
            }
        }
    }
    let release_frame = vec![
        checkpoint_account.clone(),
        account(accounts, MARKET)?.clone(),
        account(accounts, CACHE)?.clone(),
        account(accounts, REGISTRY)?.clone(),
        trading.clone(),
        account(accounts, TRADING_PROGRAMDATA)?.clone(),
        account(accounts, REALM)?.clone(),
        account(accounts, REALM_STAGING)?.clone(),
        account(accounts, REPLAY)?.clone(),
    ];
    let market = authenticate_market(&release_frame, custody)?;
    authenticate_calling_release(program_id, &release_frame, custody, None, market.cache_bump)?;
    authenticate_realm(program_id, &release_frame, custody, market.state)?;
    authenticate_replay_identity(program_id, account(accounts, REPLAY)?, custody)?;
    let replay = read_replay(account(accounts, REPLAY)?)?;
    let expected_revision = replay
        .next_revision
        .checked_add(u64::from(ordinal))
        .ok_or(CustodySbfError::Replay)?;
    if custody.expected_revision != expected_revision {
        return Err(CustodySbfError::Replay.into());
    }
    Ok(Box::new(AuthenticatedEffectV1 {
        input: checkpoint.input,
        checkpoint_reservation_receipt: checkpoint.reservation_receipt,
        checkpoint_digest: checkpoint.digest,
        effect_count: effect.effect_count,
        source_after: effect.source_after,
        destination_after: effect.destination_after,
        effect_digest: effect.effect_digest,
        effects_digest: manifest.effects_digest,
        custody,
        slot,
    }))
}

#[inline(never)]
fn read_checkpoint_facts(
    trading_program: &Pubkey,
    checkpoint_account: &AccountInfo<'_>,
    ordinal: u8,
) -> Result<Box<CheckpointFactsV1>, ProgramError> {
    if checkpoint_account.owner != trading_program
        || checkpoint_account.data_len() != DEALER_SCENARIO_CHECKPOINT_BYTES_V1
    {
        return Err(CustodySbfError::Release.into());
    }
    let data = checkpoint_account
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Release)?;
    let digest = hash(&data).to_bytes();
    let checkpoint =
        DealerScenarioCheckpointV1::decode(&data).map_err(|_| CustodySbfError::Release)?;
    let input = checkpoint.input();
    if Pubkey::find_program_address(
        &[
            DEALER_SCENARIO_CHECKPOINT_PDA_DOMAIN_V1,
            &input.request_digest,
        ],
        trading_program,
    )
    .0 != *checkpoint_account.key
    {
        return Err(CustodySbfError::Release.into());
    }
    let evaluation = checkpoint.evaluation();
    Ok(Box::new(CheckpointFactsV1 {
        input,
        digest,
        phase: checkpoint.phase(),
        reservation_count: checkpoint.reservation_count(),
        rollback_count: checkpoint.rollback_count(),
        reservation_receipt: checkpoint
            .reservation_receipt_digest(ordinal)
            .unwrap_or([0; 32]),
        effect_count: evaluation.custody_effect_count,
        effects_digest: evaluation.effects_digest,
    }))
}

#[inline(never)]
fn read_manifest_facts(
    producer: &AccountInfo<'_>,
    manifest_account: &AccountInfo<'_>,
    effect_account: &AccountInfo<'_>,
    checkpoint: &Pubkey,
    request_digest: [u8; 32],
    ordinal: u8,
) -> Result<Box<ManifestFactsV1>, ProgramError> {
    if manifest_account.owner != producer.key
        || effect_account.owner != producer.key
        || manifest_account.data_len() != DEALER_SCENARIO_CUSTODY_EFFECT_MANIFEST_BYTES_V1
    {
        return Err(CustodySbfError::Release.into());
    }
    let data = manifest_account
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Release)?;
    let effects_digest = hash(&data).to_bytes();
    let manifest = DealerScenarioCustodyEffectManifestV1::decode(&data)
        .map_err(|_| CustodySbfError::Release)?;
    let index = usize::from(ordinal);
    let effect_digest = manifest
        .effect_digests
        .get(index)
        .copied()
        .ok_or(CustodySbfError::Release)?;
    if manifest.producer_program != producer.key.to_bytes()
        || manifest.checkpoint != checkpoint.to_bytes()
        || manifest.request_digest != request_digest
        || manifest.effect_accounts.get(index).copied() != Some(effect_account.key.to_bytes())
    {
        return Err(CustodySbfError::Release.into());
    }
    Ok(Box::new(ManifestFactsV1 {
        effect_count: manifest.effect_count,
        effects_digest,
        effect_digest,
    }))
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn read_effect_facts(
    producer: &AccountInfo<'_>,
    effect_account: &AccountInfo<'_>,
    checkpoint: &Pubkey,
    request_digest: [u8; 32],
    ordinal: u8,
    effect_count: u8,
    expected_digest: [u8; 32],
) -> Result<Box<DecodedEffectV1>, ProgramError> {
    if effect_account.owner != producer.key
        || effect_account.data_len() != DEALER_SCENARIO_CUSTODY_EFFECT_BYTES_V1
    {
        return Err(CustodySbfError::Release.into());
    }
    let data = effect_account
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Release)?;
    let effect_digest = hash(&data).to_bytes();
    let effect =
        DealerScenarioCustodyEffectV1::decode(&data).map_err(|_| CustodySbfError::Release)?;
    if effect_digest != expected_digest
        || effect.producer_program != producer.key.to_bytes()
        || effect.checkpoint != checkpoint.to_bytes()
        || effect.request_digest != request_digest
        || effect.ordinal != ordinal
        || effect.effect_count != effect_count
    {
        return Err(CustodySbfError::Release.into());
    }
    let custody = match effect.kind {
        DealerScenarioCustodyRequestKindV1::Canonical => {
            CustodyRequestV1::decode(effect.request_bytes())
                .map_err(|_| CustodySbfError::Instruction)?
        }
        DealerScenarioCustodyRequestKindV1::Delegated => {
            return Err(CustodySbfError::Instruction.into());
        }
    };
    require_reversible_staged_source(effect.kind, custody)?;
    Ok(Box::new(DecodedEffectV1 {
        effect_count,
        source_after: effect.source_after,
        destination_after: effect.destination_after,
        effect_digest,
        request_wire_digest: hash(effect.request_bytes()).to_bytes(),
        custody,
    }))
}

#[inline(never)]
fn reserve(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    ordinal: u8,
    authenticated: &AuthenticatedEffectV1,
) -> Result<(), ProgramError> {
    let checkpoint_key = account(accounts, CHECKPOINT)?.key.to_bytes();
    let batch_account = account(accounts, BATCH)?;
    let state_account = account(accounts, RESERVATION_STATE)?;
    let receipt_account = account(accounts, RECEIPT)?;
    require_reservation_identities(
        program_id,
        accounts,
        authenticated.custody,
        ordinal,
        DealerScenarioReservationActionV1::Reserve,
    )?;
    require_vacant(state_account)?;
    require_vacant(receipt_account)?;
    let replay_data = account(accounts, REPLAY)?
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Replay)?;
    let replay_digest = hash(&replay_data).to_bytes();
    drop(replay_data);
    let input = authenticated.input;
    let (batch, batch_prestate_digest) = if ordinal == 0 {
        require_vacant(batch_account)?;
        (
            Box::new(
                DealerScenarioReservationBatchV1::new(
                    authenticated.effect_count,
                    input.release_set,
                    input.market,
                    authenticated.custody.realm,
                    account(accounts, TRADING_PROGRAM)?.key.to_bytes(),
                    checkpoint_key,
                    input.request_digest,
                    authenticated.effects_digest,
                    account(accounts, REPLAY)?.key.to_bytes(),
                    replay_digest,
                    input.refund_beneficiary,
                    input.expires_at,
                    input.generation,
                )
                .map_err(|_| CustodySbfError::Replay)?,
            ),
            vacant_digest(),
        )
    } else {
        let (batch, digest) = read_batch(program_id, batch_account, checkpoint_key)?;
        if batch.reserved_count != ordinal
            || batch.replay_prestate_digest != replay_digest
            || batch.effects_digest != authenticated.effects_digest
        {
            return Err(CustodySbfError::Replay.into());
        }
        (batch, digest)
    };

    let state = execute_reserve_token_effect(
        program_id,
        accounts,
        ordinal,
        authenticated,
        batch_account.key.to_bytes(),
        checkpoint_key,
    )?;
    let rent =
        Rent::from_account_info(account(accounts, RENT)?).map_err(|_| CustodySbfError::Create)?;
    create_state_account(program_id, accounts, ordinal, &rent)?;
    let state_digest = write_state_value(state_account, &state)?;
    let receipt = Box::new(DealerScenarioReservationReceiptV1 {
        action: DealerScenarioReservationActionV1::Reserve,
        effect_ordinal: ordinal,
        effect_count: authenticated.effect_count,
        producer_program: program_id.to_bytes(),
        checkpoint: checkpoint_key,
        checkpoint_prestate_digest: authenticated.checkpoint_digest,
        request_digest: input.request_digest,
        effects_digest: authenticated.effects_digest,
        reservation: state_account.key.to_bytes(),
        reservation_prestate_digest: vacant_digest(),
        reservation_poststate_digest: state_digest,
        prior_receipt_digest: [0; 32],
    });
    create_receipt_account(
        program_id,
        accounts,
        ordinal,
        DealerScenarioReservationActionV1::Reserve,
        &rent,
    )?;
    let receipt_digest = write_receipt_value(receipt_account, &receipt)?;
    let next = Box::new(
        batch
            .append_reserve(
                authenticated.slot,
                ordinal,
                batch_prestate_digest,
                state_account.key.to_bytes(),
                receipt_digest,
            )
            .map_err(|_| CustodySbfError::Replay)?,
    );
    if ordinal == 0 {
        create_batch_account(program_id, accounts, &rent)?;
    }
    write_batch_value(batch_account, &next)?;
    set_account_return_data(receipt_account)?;
    Ok(())
}

#[inline(never)]
fn execute_reserve_token_effect(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    ordinal: u8,
    authenticated: &AuthenticatedEffectV1,
    batch: [u8; 32],
    checkpoint: [u8; 32],
) -> Result<Box<DealerScenarioReservationStateV1>, ProgramError> {
    let original = authenticated.custody;
    let realm_frame = vec![
        account(accounts, CHECKPOINT)?.clone(),
        account(accounts, MARKET)?.clone(),
        account(accounts, CACHE)?.clone(),
        account(accounts, REGISTRY)?.clone(),
        account(accounts, TRADING_PROGRAM)?.clone(),
        account(accounts, TRADING_PROGRAMDATA)?.clone(),
        account(accounts, REALM)?.clone(),
        account(accounts, REALM_STAGING)?.clone(),
        account(accounts, REPLAY)?.clone(),
    ];
    let market = authenticate_market(&realm_frame, original)?;
    let realm = authenticate_realm(program_id, &realm_frame, original, market.state)?;
    let mint = account(accounts, MINT)?;
    let source = account(accounts, SOURCE)?;
    let destination = account(accounts, DESTINATION)?;
    let escrow = account(accounts, ESCROW)?;
    let authority = account(accounts, CUSTODY_AUTHORITY)?;
    let token_program = account(accounts, TOKEN_PROGRAM)?;
    validate_token_program_and_mint(mint, token_program, original, realm)?;
    let authority_bump = validate_custody_authority(program_id, authority, original)?;
    if source.owner != token_program.key || destination.owner != token_program.key {
        return Err(CustodySbfError::TokenState.into());
    }
    if original.source_compartment != CompartmentV1::External {
        validate_vault_key(program_id, source, original, true)?;
    }
    if original.destination_compartment != CompartmentV1::External {
        validate_vault_key(program_id, destination, original, false)?;
    }
    let original_accounts = TransferAccounts {
        source,
        destination,
        mint,
        authority,
        token_program,
    };
    let original_before =
        authenticate_transfer_accounts(original_accounts, original, realm.profile, true)?;
    let source_prestate_digest = account_digest(source)?;
    let destination_prestate_digest = account_digest(destination)?;
    if original_before.source.checked_sub(original.amount) != Some(authenticated.source_after)
        || original_before.destination.checked_add(original.amount)
            != Some(authenticated.destination_after)
    {
        return Err(CustodySbfError::Postcondition.into());
    }
    let reserve_request = Box::new(reserve_request(
        original,
        *account(accounts, RESERVATION_STATE)?.key,
        *escrow.key,
    ));
    reserve_request
        .validate()
        .map_err(|_| CustodySbfError::Instruction)?;
    validate_vault_key(program_id, escrow, *reserve_request, false)?;
    require_vacant(escrow)?;
    let rent =
        Rent::from_account_info(account(accounts, RENT)?).map_err(|_| CustodySbfError::Create)?;
    create_vault(
        program_id,
        account(accounts, PAYER)?,
        escrow,
        account(accounts, SYSTEM)?,
        token_program,
        *reserve_request,
        rent.minimum_balance(dclutch_token_svm::ACCOUNT_BYTES),
    )?;
    initialize_vault(escrow, mint, authority, token_program, *reserve_request)?;
    let reserve_accounts = TransferAccounts {
        source,
        destination: escrow,
        mint,
        authority,
        token_program,
    };
    let before =
        authenticate_transfer_accounts(reserve_accounts, *reserve_request, realm.profile, true)?;
    invoke_exact_transfer(
        reserve_accounts,
        *reserve_request,
        before.decimals,
        authority_bump,
    )?;
    let after =
        authenticate_transfer_accounts(reserve_accounts, *reserve_request, realm.profile, false)?;
    if after.source != authenticated.source_after
        || after.destination != original.amount
        || before.destination != 0
        || before.source != original_before.source
    {
        return Err(CustodySbfError::Postcondition.into());
    }
    Ok(Box::new(DealerScenarioReservationStateV1 {
        status: DealerScenarioReservationStateStatusV1::Active,
        ordinal,
        effect_count: authenticated.effect_count,
        batch,
        checkpoint,
        request_digest: authenticated.input.request_digest,
        effects_digest: authenticated.effects_digest,
        effect_digest: authenticated.effect_digest,
        source: source.key.to_bytes(),
        destination: destination.key.to_bytes(),
        escrow: escrow.key.to_bytes(),
        mint: mint.key.to_bytes(),
        token_program: token_program.key.to_bytes(),
        source_prestate_digest,
        destination_prestate_digest,
        effect_poststate_digest: account_digest(escrow)?,
        source_poststate_digest: account_digest(source)?,
        amount: original.amount,
        source_after: after.source,
        destination_before: original_before.destination,
        escrow_after: after.destination,
    }))
}

#[inline(never)]
fn rollback(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    ordinal: u8,
    authenticated: &AuthenticatedEffectV1,
) -> Result<(), ProgramError> {
    require_reservation_identities(
        program_id,
        accounts,
        authenticated.custody,
        ordinal,
        DealerScenarioReservationActionV1::Rollback,
    )?;
    require_vacant(account(accounts, RECEIPT)?)?;
    let checkpoint_key = account(accounts, CHECKPOINT)?.key.to_bytes();
    let (batch, batch_prestate_digest) =
        read_batch(program_id, account(accounts, BATCH)?, checkpoint_key)?;
    let state_account = account(accounts, RESERVATION_STATE)?;
    let state_data = state_account
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Replay)?;
    let state_prestate_digest = hash(&state_data).to_bytes();
    let mut state = Box::new(
        DealerScenarioReservationStateV1::decode(&state_data)
            .map_err(|_| CustodySbfError::Replay)?,
    );
    drop(state_data);
    let prior_receipt = batch
        .receipt_digests
        .get(usize::from(ordinal))
        .copied()
        .ok_or(CustodySbfError::Replay)?;
    if state_account.owner != program_id
        || state.status != DealerScenarioReservationStateStatusV1::Active
        || state.batch != account(accounts, BATCH)?.key.to_bytes()
        || state.checkpoint != checkpoint_key
        || state.effect_digest != authenticated.effect_digest
        || state.source != account(accounts, SOURCE)?.key.to_bytes()
        || state.destination != account(accounts, DESTINATION)?.key.to_bytes()
        || state.escrow != account(accounts, ESCROW)?.key.to_bytes()
        || state.mint != account(accounts, MINT)?.key.to_bytes()
        || state.token_program != account(accounts, TOKEN_PROGRAM)?.key.to_bytes()
        || authenticated.checkpoint_reservation_receipt != prior_receipt
    {
        return Err(CustodySbfError::Replay.into());
    }
    state = execute_rollback_token_effect(program_id, accounts, authenticated, state)?;
    let state_poststate_digest = write_state_value(state_account, &state)?;
    let receipt = Box::new(DealerScenarioReservationReceiptV1 {
        action: DealerScenarioReservationActionV1::Rollback,
        effect_ordinal: ordinal,
        effect_count: state.effect_count,
        producer_program: program_id.to_bytes(),
        checkpoint: checkpoint_key,
        checkpoint_prestate_digest: authenticated.checkpoint_digest,
        request_digest: authenticated.input.request_digest,
        effects_digest: authenticated.effects_digest,
        reservation: state_account.key.to_bytes(),
        reservation_prestate_digest: state_prestate_digest,
        reservation_poststate_digest: state_poststate_digest,
        prior_receipt_digest: prior_receipt,
    });
    let rent =
        Rent::from_account_info(account(accounts, RENT)?).map_err(|_| CustodySbfError::Create)?;
    create_receipt_account(
        program_id,
        accounts,
        ordinal,
        DealerScenarioReservationActionV1::Rollback,
        &rent,
    )?;
    let receipt_digest = write_receipt_value(account(accounts, RECEIPT)?, &receipt)?;
    let next = Box::new(
        batch
            .append_rollback(
                authenticated.slot,
                ordinal,
                batch_prestate_digest,
                prior_receipt,
                receipt_digest,
            )
            .map_err(|_| CustodySbfError::Replay)?,
    );
    write_batch_value(account(accounts, BATCH)?, &next)?;
    set_account_return_data(account(accounts, RECEIPT)?)?;
    Ok(())
}

#[inline(never)]
fn execute_rollback_token_effect(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    authenticated: &AuthenticatedEffectV1,
    mut state: Box<DealerScenarioReservationStateV1>,
) -> Result<Box<DealerScenarioReservationStateV1>, ProgramError> {
    let original = authenticated.custody;
    let realm_frame = vec![
        account(accounts, CHECKPOINT)?.clone(),
        account(accounts, MARKET)?.clone(),
        account(accounts, CACHE)?.clone(),
        account(accounts, REGISTRY)?.clone(),
        account(accounts, TRADING_PROGRAM)?.clone(),
        account(accounts, TRADING_PROGRAMDATA)?.clone(),
        account(accounts, REALM)?.clone(),
        account(accounts, REALM_STAGING)?.clone(),
        account(accounts, REPLAY)?.clone(),
    ];
    let market = authenticate_market(&realm_frame, original)?;
    let realm = authenticate_realm(program_id, &realm_frame, original, market.state)?;
    let source = account(accounts, SOURCE)?;
    let escrow = account(accounts, ESCROW)?;
    let mint = account(accounts, MINT)?;
    let authority = account(accounts, CUSTODY_AUTHORITY)?;
    let token_program = account(accounts, TOKEN_PROGRAM)?;
    let reverse = Box::new(rollback_request(
        original,
        *account(accounts, RESERVATION_STATE)?.key,
        *escrow.key,
    ));
    reverse
        .validate()
        .map_err(|_| CustodySbfError::Instruction)?;
    validate_token_program_and_mint(mint, token_program, *reverse, realm)?;
    let authority_bump = validate_custody_authority(program_id, authority, *reverse)?;
    validate_vault_key(program_id, escrow, *reverse, true)?;
    if original.source_compartment != CompartmentV1::External {
        validate_vault_key(program_id, source, *reverse, false)?;
    }
    let reverse_accounts = TransferAccounts {
        source: escrow,
        destination: source,
        mint,
        authority,
        token_program,
    };
    let before = authenticate_transfer_accounts(reverse_accounts, *reverse, realm.profile, true)?;
    if before.source != state.amount || before.destination != state.source_after {
        return Err(CustodySbfError::Postcondition.into());
    }
    invoke_exact_transfer(reverse_accounts, *reverse, before.decimals, authority_bump)?;
    let after = authenticate_transfer_accounts(reverse_accounts, *reverse, realm.profile, false)?;
    if after.source != 0 || after.destination.checked_sub(state.amount) != Some(state.source_after)
    {
        return Err(CustodySbfError::Postcondition.into());
    }
    let escrow_lamports = escrow.lamports();
    let refund = account(accounts, REFUND)?;
    let refund_before = refund.lamports();
    let mut close_request = *reverse;
    close_request.rent_refund = refund.key.to_bytes();
    invoke_close(
        escrow,
        refund,
        authority,
        token_program,
        close_request,
        authority_bump,
    )?;
    if escrow.lamports() != 0
        || refund.lamports()
            != refund_before
                .checked_add(escrow_lamports)
                .ok_or(CustodySbfError::Postcondition)?
    {
        return Err(CustodySbfError::Postcondition.into());
    }
    state.status = DealerScenarioReservationStateStatusV1::RolledBack;
    state.source_poststate_digest = account_digest(source)?;
    state.effect_poststate_digest = account_digest(source)?;
    state.escrow_after = 0;
    Ok(state)
}

/// Atomically deliver every reserved effect, close every temporary escrow,
/// advance the standard Custody replay cursor, and persist one typed receipt.
#[inline(never)]
fn activate_batch(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    effect_count: u8,
) -> Result<(), ProgramError> {
    require_activation_frame(accounts, effect_count)?;
    let trading = account(accounts, TRADING_PROGRAM)?;
    let checkpoint_account = account(accounts, ACT_CHECKPOINT)?;
    let first_checkpoint = read_checkpoint_facts(trading.key, checkpoint_account, 0)?;
    require_committed_checkpoint(&first_checkpoint, effect_count)?;
    let checkpoint_key = checkpoint_account.key.to_bytes();
    let (batch, batch_prestate_digest) =
        read_batch(program_id, account(accounts, ACT_BATCH)?, checkpoint_key)?;
    if batch.status != DealerScenarioReservationBatchStatusV1::Reserved
        || batch.effect_count != effect_count
        || batch.reserved_count != effect_count
        || batch.rollback_count != 0
        || batch.release_set != first_checkpoint.input.release_set
        || batch.market != first_checkpoint.input.market
        || batch.trading_program != trading.key.to_bytes()
        || batch.request_digest != first_checkpoint.input.request_digest
        || batch.effects_digest != first_checkpoint.effects_digest
        || batch.replay != account(accounts, ACT_REPLAY)?.key.to_bytes()
        || batch.refund_beneficiary != account(accounts, ACT_REFUND)?.key.to_bytes()
        || batch.expires_at != first_checkpoint.input.expires_at
        || batch.generation != first_checkpoint.input.generation
    {
        return Err(CustodySbfError::Replay.into());
    }
    require_activation_identities(program_id, accounts, first_checkpoint.input.request_digest)?;
    require_vacant(account(accounts, ACT_RECEIPT)?)?;

    let first_effect = read_activation_effect(
        accounts,
        checkpoint_account.key,
        first_checkpoint.input.request_digest,
        effect_count,
        0,
    )?;
    authenticate_activation_release(program_id, accounts, &first_checkpoint, &first_effect)?;
    let replay_account = account(accounts, ACT_REPLAY)?;
    let replay_data = replay_account
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Replay)?;
    let replay_prestate_digest = hash(&replay_data).to_bytes();
    drop(replay_data);
    if replay_prestate_digest != batch.replay_prestate_digest {
        return Err(CustodySbfError::Replay.into());
    }
    let mut replay = Box::new(read_replay(replay_account)?);

    for ordinal in 0..effect_count {
        let checkpoint = read_checkpoint_facts(trading.key, checkpoint_account, ordinal)?;
        let effect = read_activation_effect(
            accounts,
            checkpoint_account.key,
            first_checkpoint.input.request_digest,
            effect_count,
            ordinal,
        )?;
        require_activation_effect_join(
            accounts,
            &first_checkpoint,
            &checkpoint,
            &batch,
            &effect,
            ordinal,
        )?;
        let poststate = activate_one_effect(program_id, accounts, &effect, ordinal)?;
        *replay = replay
            .advance(effect.custody, effect.request_wire_digest, poststate)
            .map_err(|_| CustodySbfError::Replay)?;
    }

    finish_activation(
        program_id,
        accounts,
        &first_checkpoint,
        batch,
        batch_prestate_digest,
        replay,
        replay_prestate_digest,
    )
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn finish_activation(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    checkpoint: &CheckpointFactsV1,
    batch: Box<DealerScenarioReservationBatchV1>,
    batch_prestate_digest: [u8; 32],
    replay: Box<CustodyReplayV1>,
    replay_prestate_digest: [u8; 32],
) -> Result<(), ProgramError> {
    let replay_account = account(accounts, ACT_REPLAY)?;
    let replay_bytes = replay.to_bytes().map_err(|_| CustodySbfError::Replay)?;
    if replay_bytes.len() != CUSTODY_REPLAY_BYTES_V1 {
        return Err(CustodySbfError::Commit.into());
    }
    write_exact(replay_account, &replay_bytes)?;
    let replay_poststate_digest = hash(&replay_bytes).to_bytes();
    let activated = Box::new(
        batch
            .activate_committed(batch_prestate_digest)
            .map_err(|_| CustodySbfError::Replay)?,
    );
    let batch_poststate_digest = write_batch_value(account(accounts, ACT_BATCH)?, &activated)?;
    let receipt = Box::new(DealerScenarioActivationReceiptV1 {
        producer_program: program_id.to_bytes(),
        checkpoint: account(accounts, ACT_CHECKPOINT)?.key.to_bytes(),
        checkpoint_prestate_digest: checkpoint.digest,
        request_digest: checkpoint.input.request_digest,
        effects_digest: checkpoint.effects_digest,
        batch: account(accounts, ACT_BATCH)?.key.to_bytes(),
        batch_prestate_digest,
        batch_poststate_digest,
        replay_prestate_digest,
        replay_poststate_digest,
    });
    let rent = Rent::from_account_info(account(accounts, ACT_RENT)?)
        .map_err(|_| CustodySbfError::Create)?;
    create_activation_receipt_account(
        program_id,
        accounts,
        checkpoint.input.request_digest,
        &rent,
    )?;
    let receipt_bytes = receipt.encode().map_err(|_| CustodySbfError::Commit)?;
    write_exact(account(accounts, ACT_RECEIPT)?, &receipt_bytes)?;
    set_return_data(&receipt_bytes);
    Ok(())
}

#[inline(never)]
fn require_activation_frame(
    accounts: &[AccountInfo<'_>],
    effect_count: u8,
) -> Result<(), ProgramError> {
    let expected = ACT_COMMON_ACCOUNT_COUNT
        .checked_add(
            usize::from(effect_count)
                .checked_mul(ACT_EFFECT_ACCOUNT_COUNT)
                .ok_or(CustodySbfError::AccountFrame)?,
        )
        .ok_or(CustodySbfError::AccountFrame)?;
    if accounts.len() != expected {
        return Err(CustodySbfError::AccountFrame.into());
    }
    let common_writable = [ACT_REPLAY, ACT_BATCH, ACT_RECEIPT, ACT_PAYER, ACT_REFUND];
    let common_executable = [
        REGISTRY,
        TRADING_PROGRAM,
        ACT_EFFECT_PRODUCER,
        ACT_TOKEN_PROGRAM,
        ACT_SYSTEM,
    ];
    for (index, current) in accounts.iter().enumerate() {
        let effect_offset = index.checked_sub(ACT_COMMON_ACCOUNT_COUNT);
        let effect_role = effect_offset.map(|offset| offset % ACT_EFFECT_ACCOUNT_COUNT);
        let expected_writable =
            common_writable.contains(&index) || matches!(effect_role, Some(1 | 2 | 3));
        let expected_signer = index == ACT_PAYER;
        let expected_executable = common_executable.contains(&index);
        if current.is_writable != expected_writable
            || current.is_signer != expected_signer
            || current.executable != expected_executable
        {
            return Err(CustodySbfError::AccountFrame.into());
        }
    }
    // The effect producer is not an independent identity. Trading's commit
    // route refuses any effect manifest whose `producer_program` is not the
    // Trading program itself, so every committed batch reaching delivery names
    // Trading as its producer; the frame carries the same key twice by
    // construction. Pin that equality positively and take the producer slot out
    // of the duplicate census, because a frame that must repeat one key and is
    // also forbidden to repeat any key cannot be built at all.
    if account(accounts, ACT_EFFECT_PRODUCER)?.key != account(accounts, TRADING_PROGRAM)?.key {
        return Err(CustodySbfError::AccountFrame.into());
    }
    if account(accounts, ACT_RENT)?.key != &sysvar::rent::ID
        || account(accounts, ACT_SYSTEM)?.key != &system_program::ID
        || account(accounts, REGISTRY)?.key == account(accounts, TRADING_PROGRAM)?.key
        || has_duplicate_keys_except(accounts, ACT_EFFECT_PRODUCER)
    {
        return Err(CustodySbfError::AccountFrame.into());
    }
    Ok(())
}

fn require_committed_checkpoint(
    checkpoint: &CheckpointFactsV1,
    effect_count: u8,
) -> Result<(), ProgramError> {
    if checkpoint.phase != DealerScenarioCheckpointPhaseV1::Committed
        || checkpoint.effect_count != effect_count
        || checkpoint.reservation_count != effect_count
        || checkpoint.rollback_count != 0
    {
        return Err(CustodySbfError::Replay.into());
    }
    Ok(())
}

fn activation_effect_index(ordinal: u8) -> Result<usize, ProgramError> {
    ACT_COMMON_ACCOUNT_COUNT
        .checked_add(
            usize::from(ordinal)
                .checked_mul(ACT_EFFECT_ACCOUNT_COUNT)
                .ok_or(CustodySbfError::AccountFrame)?,
        )
        .ok_or_else(|| CustodySbfError::AccountFrame.into())
}

#[inline(never)]
fn read_activation_effect(
    accounts: &[AccountInfo<'_>],
    checkpoint: &Pubkey,
    request_digest: [u8; 32],
    effect_count: u8,
    ordinal: u8,
) -> Result<Box<DecodedEffectV1>, ProgramError> {
    let effect_index = activation_effect_index(ordinal)?;
    let producer = account(accounts, ACT_EFFECT_PRODUCER)?;
    let manifest = read_manifest_facts(
        producer,
        account(accounts, ACT_MANIFEST)?,
        account(accounts, effect_index)?,
        checkpoint,
        request_digest,
        ordinal,
    )?;
    if manifest.effect_count != effect_count {
        return Err(CustodySbfError::Release.into());
    }
    read_effect_facts(
        producer,
        account(accounts, effect_index)?,
        checkpoint,
        request_digest,
        ordinal,
        effect_count,
        manifest.effect_digest,
    )
}

#[inline(never)]
fn authenticate_activation_release(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    checkpoint: &CheckpointFactsV1,
    effect: &DecodedEffectV1,
) -> Result<(), ProgramError> {
    let request = effect.custody;
    if request.operation != OperationV1::Transfer
        || request.caller_role != ExecutionRoleV1::Trading
        || request.caller_program != account(accounts, TRADING_PROGRAM)?.key.to_bytes()
        || request.release_set != checkpoint.input.release_set
        || request.market != checkpoint.input.market
        || request.semantic.parent_request_digest != checkpoint.input.request_digest
        || request.semantic.generation != checkpoint.input.generation
    {
        return Err(CustodySbfError::Release.into());
    }
    let release_frame = vec![
        account(accounts, ACT_CHECKPOINT)?.clone(),
        account(accounts, MARKET)?.clone(),
        account(accounts, CACHE)?.clone(),
        account(accounts, REGISTRY)?.clone(),
        account(accounts, TRADING_PROGRAM)?.clone(),
        account(accounts, TRADING_PROGRAMDATA)?.clone(),
        account(accounts, REALM)?.clone(),
        account(accounts, REALM_STAGING)?.clone(),
        account(accounts, ACT_REPLAY)?.clone(),
    ];
    let market = authenticate_market(&release_frame, request)?;
    authenticate_calling_release(program_id, &release_frame, request, None, market.cache_bump)?;
    let realm = authenticate_realm(program_id, &release_frame, request, market.state)?;
    authenticate_replay_identity(program_id, account(accounts, ACT_REPLAY)?, request)?;
    validate_token_program_and_mint(
        account(accounts, ACT_MINT)?,
        account(accounts, ACT_TOKEN_PROGRAM)?,
        request,
        realm,
    )?;
    validate_custody_authority(
        program_id,
        account(accounts, ACT_CUSTODY_AUTHORITY)?,
        request,
    )?;
    Ok(())
}

fn require_activation_effect_join(
    accounts: &[AccountInfo<'_>],
    first_checkpoint: &CheckpointFactsV1,
    checkpoint: &CheckpointFactsV1,
    batch: &DealerScenarioReservationBatchV1,
    effect: &DecodedEffectV1,
    ordinal: u8,
) -> Result<(), ProgramError> {
    let state_index = activation_effect_index(ordinal)?
        .checked_add(1)
        .ok_or(CustodySbfError::AccountFrame)?;
    let original = effect.custody;
    let reservation_state = batch
        .reservation_states
        .get(usize::from(ordinal))
        .copied()
        .ok_or(CustodySbfError::Replay)?;
    let receipt_digest = batch
        .receipt_digests
        .get(usize::from(ordinal))
        .copied()
        .ok_or(CustodySbfError::Replay)?;
    if checkpoint.input != first_checkpoint.input
        || checkpoint.digest != first_checkpoint.digest
        || checkpoint.phase != DealerScenarioCheckpointPhaseV1::Committed
        || checkpoint.reservation_count != batch.effect_count
        || checkpoint.rollback_count != 0
        || checkpoint.reservation_receipt != receipt_digest
        || effect.effect_count != batch.effect_count
        || original.release_set != batch.release_set
        || original.market != batch.market
        || original.realm != batch.realm
        || original.caller_program != batch.trading_program
        || original.semantic.parent_request_digest != batch.request_digest
        || original.semantic.generation != batch.generation
        || original.semantic.transfer_index != u16::from(ordinal)
        || original.mint != account(accounts, ACT_MINT)?.key.to_bytes()
        || original.token_program != account(accounts, ACT_TOKEN_PROGRAM)?.key.to_bytes()
        || account(accounts, state_index)?.key.to_bytes() != reservation_state
    {
        return Err(CustodySbfError::Replay.into());
    }
    Ok(())
}

#[inline(never)]
fn activate_one_effect(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    effect: &DecodedEffectV1,
    ordinal: u8,
) -> Result<[u8; 32], ProgramError> {
    let effect_index = activation_effect_index(ordinal)?;
    let state = account(accounts, effect_index + 1)?;
    let escrow = account(accounts, effect_index + 2)?;
    let destination = account(accounts, effect_index + 3)?;
    let state_data = state
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Replay)?;
    let mut reservation = Box::new(
        DealerScenarioReservationStateV1::decode(&state_data)
            .map_err(|_| CustodySbfError::Replay)?,
    );
    drop(state_data);
    let original = effect.custody;
    if state.owner != program_id
        || reservation.status != DealerScenarioReservationStateStatusV1::Active
        || reservation.ordinal != ordinal
        || reservation.effect_count != effect.effect_count
        || reservation.batch != account(accounts, ACT_BATCH)?.key.to_bytes()
        || reservation.checkpoint != account(accounts, ACT_CHECKPOINT)?.key.to_bytes()
        || reservation.request_digest != original.semantic.parent_request_digest
        || reservation.effects_digest
            != hash(
                &account(accounts, ACT_MANIFEST)?
                    .try_borrow_data()
                    .map_err(|_| CustodySbfError::Release)?,
            )
            .to_bytes()
        || reservation.effect_digest != effect.effect_digest
        || reservation.source != original.source
        || reservation.destination != destination.key.to_bytes()
        || reservation.escrow != escrow.key.to_bytes()
        || reservation.mint != account(accounts, ACT_MINT)?.key.to_bytes()
        || reservation.token_program != account(accounts, ACT_TOKEN_PROGRAM)?.key.to_bytes()
        || reservation.effect_poststate_digest != account_digest(escrow)?
    {
        return Err(CustodySbfError::Replay.into());
    }
    let request = activate_request(original, *state.key, *escrow.key);
    request
        .validate()
        .map_err(|_| CustodySbfError::Instruction)?;
    let mint = account(accounts, ACT_MINT)?;
    let authority = account(accounts, ACT_CUSTODY_AUTHORITY)?;
    let token_program = account(accounts, ACT_TOKEN_PROGRAM)?;
    validate_vault_key(program_id, escrow, request, true)?;
    if original.destination_compartment != CompartmentV1::External {
        validate_vault_key(program_id, destination, request, false)?;
    }
    let authority_bump = validate_custody_authority(program_id, authority, request)?;
    let transfer_accounts = TransferAccounts {
        source: escrow,
        destination,
        mint,
        authority,
        token_program,
    };
    let realm_frame = vec![
        account(accounts, ACT_CHECKPOINT)?.clone(),
        account(accounts, MARKET)?.clone(),
        account(accounts, CACHE)?.clone(),
        account(accounts, REGISTRY)?.clone(),
        account(accounts, TRADING_PROGRAM)?.clone(),
        account(accounts, TRADING_PROGRAMDATA)?.clone(),
        account(accounts, REALM)?.clone(),
        account(accounts, REALM_STAGING)?.clone(),
        account(accounts, ACT_REPLAY)?.clone(),
    ];
    let market = authenticate_market(&realm_frame, original)?;
    let realm = authenticate_realm(program_id, &realm_frame, original, market.state)?;
    let before = authenticate_transfer_accounts(transfer_accounts, request, realm.profile, true)?;
    if before.source != reservation.amount
        || before.destination != reservation.destination_before
        || account_digest(destination)? != reservation.destination_prestate_digest
    {
        return Err(CustodySbfError::Postcondition.into());
    }
    invoke_exact_transfer(transfer_accounts, request, before.decimals, authority_bump)?;
    let after = authenticate_transfer_accounts(transfer_accounts, request, realm.profile, false)?;
    if after.source != 0
        || after.destination.checked_sub(reservation.amount) != Some(reservation.destination_before)
    {
        return Err(CustodySbfError::Postcondition.into());
    }
    let escrow_lamports = escrow.lamports();
    let refund = account(accounts, ACT_REFUND)?;
    let refund_before = refund.lamports();
    let mut close_request = request;
    close_request.rent_refund = refund.key.to_bytes();
    invoke_close(
        escrow,
        refund,
        authority,
        token_program,
        close_request,
        authority_bump,
    )?;
    if escrow.lamports() != 0
        || refund.lamports()
            != refund_before
                .checked_add(escrow_lamports)
                .ok_or(CustodySbfError::Postcondition)?
    {
        return Err(CustodySbfError::Postcondition.into());
    }
    reservation.status = DealerScenarioReservationStateStatusV1::Activated;
    reservation.effect_poststate_digest = account_digest(destination)?;
    reservation.escrow_after = 0;
    write_state_value(state, &reservation)?;
    Ok(poststate_commitment(PoststateProjection {
        request_digest: effect.request_wire_digest,
        source: original.source,
        destination: original.destination,
        source_before: reservation
            .source_after
            .checked_add(reservation.amount)
            .ok_or(CustodySbfError::Postcondition)?,
        source_after: reservation.source_after,
        destination_before: reservation.destination_before,
        destination_after: after.destination,
        rent_lamports: 0,
    }))
}

fn require_activation_identities(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request_digest: [u8; 32],
) -> Result<(), ProgramError> {
    let checkpoint = account(accounts, ACT_CHECKPOINT)?;
    let expected_batch = Pubkey::find_program_address(
        &[
            DEALER_SCENARIO_RESERVATION_BATCH_PDA_DOMAIN_V1,
            checkpoint.key.as_ref(),
        ],
        program_id,
    )
    .0;
    let expected_receipt = Pubkey::find_program_address(
        &[
            DEALER_SCENARIO_ACTIVATION_RECEIPT_PDA_DOMAIN_V1,
            checkpoint.key.as_ref(),
            &request_digest,
        ],
        program_id,
    )
    .0;
    if account(accounts, ACT_BATCH)?.key != &expected_batch
        || account(accounts, ACT_RECEIPT)?.key != &expected_receipt
    {
        return Err(CustodySbfError::AccountFrame.into());
    }
    Ok(())
}

fn create_activation_receipt_account(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request_digest: [u8; 32],
    rent: &Rent,
) -> Result<(), ProgramError> {
    let checkpoint = account(accounts, ACT_CHECKPOINT)?;
    let bump = Pubkey::find_program_address(
        &[
            DEALER_SCENARIO_ACTIVATION_RECEIPT_PDA_DOMAIN_V1,
            checkpoint.key.as_ref(),
            &request_digest,
        ],
        program_id,
    )
    .1;
    let bump_seed = [bump];
    create_program_account(
        program_id,
        account(accounts, ACT_PAYER)?,
        account(accounts, ACT_RECEIPT)?,
        account(accounts, ACT_SYSTEM)?,
        rent,
        DEALER_SCENARIO_ACTIVATION_RECEIPT_BYTES_V1,
        &[
            DEALER_SCENARIO_ACTIVATION_RECEIPT_PDA_DOMAIN_V1,
            checkpoint.key.as_ref(),
            &request_digest,
            bump_seed.as_slice(),
        ],
    )
}

fn activate_request(request: CustodyRequestV1, state: Pubkey, escrow: Pubkey) -> CustodyRequestV1 {
    let mut activated = request;
    activated.source = escrow.to_bytes();
    activated.source_compartment = CompartmentV1::RecoveryReserve;
    activated.source_vault_context = state.to_bytes();
    activated.semantic.source_owner = [0; 32];
    activated
}

fn reserve_request(
    mut request: CustodyRequestV1,
    state: Pubkey,
    escrow: Pubkey,
) -> CustodyRequestV1 {
    request.destination = escrow.to_bytes();
    request.destination_compartment = CompartmentV1::RecoveryReserve;
    request.destination_vault_context = state.to_bytes();
    request.semantic.destination_owner = [0; 32];
    request
}

fn rollback_request(request: CustodyRequestV1, state: Pubkey, escrow: Pubkey) -> CustodyRequestV1 {
    let mut reverse = request;
    reverse.source = escrow.to_bytes();
    reverse.source_compartment = CompartmentV1::RecoveryReserve;
    reverse.source_vault_context = state.to_bytes();
    reverse.destination = request.source;
    reverse.destination_compartment = request.source_compartment;
    reverse.destination_vault_context = request.source_vault_context;
    reverse.semantic.source_owner = [0; 32];
    reverse.semantic.destination_owner = request.semantic.source_owner;
    reverse
}

fn require_reservation_identities(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CustodyRequestV1,
    ordinal: u8,
    action: DealerScenarioReservationActionV1,
) -> Result<(), ProgramError> {
    let checkpoint = account(accounts, CHECKPOINT)?;
    let ordinal_seed = [ordinal];
    let expected_batch = Pubkey::find_program_address(
        &[
            DEALER_SCENARIO_RESERVATION_BATCH_PDA_DOMAIN_V1,
            checkpoint.key.as_ref(),
        ],
        program_id,
    )
    .0;
    let expected_state = Pubkey::find_program_address(
        &[
            DEALER_SCENARIO_RESERVATION_STATE_PDA_DOMAIN_V1,
            checkpoint.key.as_ref(),
            ordinal_seed.as_slice(),
        ],
        program_id,
    )
    .0;
    let action_seed = [action as u8];
    let expected_receipt = Pubkey::find_program_address(
        &[
            DEALER_SCENARIO_RESERVATION_RECEIPT_PDA_DOMAIN_V1,
            checkpoint.key.as_ref(),
            &request.semantic.parent_request_digest,
            action_seed.as_slice(),
            ordinal_seed.as_slice(),
        ],
        program_id,
    )
    .0;
    let reserve = reserve_request(request, expected_state, *account(accounts, ESCROW)?.key);
    let expected_escrow = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::from_request(reserve, false).as_slices(),
        program_id,
    )
    .0;
    if account(accounts, BATCH)?.key != &expected_batch
        || account(accounts, RESERVATION_STATE)?.key != &expected_state
        || account(accounts, RECEIPT)?.key != &expected_receipt
        || account(accounts, ESCROW)?.key != &expected_escrow
    {
        return Err(CustodySbfError::AccountFrame.into());
    }
    Ok(())
}

fn read_batch(
    program_id: &Pubkey,
    batch: &AccountInfo<'_>,
    checkpoint: [u8; 32],
) -> Result<(Box<DealerScenarioReservationBatchV1>, [u8; 32]), ProgramError> {
    let expected = Pubkey::find_program_address(
        &[DEALER_SCENARIO_RESERVATION_BATCH_PDA_DOMAIN_V1, &checkpoint],
        program_id,
    )
    .0;
    if batch.key != &expected
        || batch.owner != program_id
        || batch.data_len() != DEALER_SCENARIO_RESERVATION_BATCH_BYTES_V1
    {
        return Err(CustodySbfError::Replay.into());
    }
    let data = batch
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Replay)?;
    let digest = hash(&data).to_bytes();
    let value =
        DealerScenarioReservationBatchV1::decode(&data).map_err(|_| CustodySbfError::Replay)?;
    Ok((Box::new(value), digest))
}

fn create_batch_account(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    rent: &Rent,
) -> Result<(), ProgramError> {
    let checkpoint = account(accounts, CHECKPOINT)?;
    let batch = account(accounts, BATCH)?;
    let bump = Pubkey::find_program_address(
        &[
            DEALER_SCENARIO_RESERVATION_BATCH_PDA_DOMAIN_V1,
            checkpoint.key.as_ref(),
        ],
        program_id,
    )
    .1;
    let bump_seed = [bump];
    create_program_account(
        program_id,
        account(accounts, PAYER)?,
        batch,
        account(accounts, SYSTEM)?,
        rent,
        DEALER_SCENARIO_RESERVATION_BATCH_BYTES_V1,
        &[
            DEALER_SCENARIO_RESERVATION_BATCH_PDA_DOMAIN_V1,
            checkpoint.key.as_ref(),
            bump_seed.as_slice(),
        ],
    )
}

fn create_state_account(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    ordinal: u8,
    rent: &Rent,
) -> Result<(), ProgramError> {
    let checkpoint = account(accounts, CHECKPOINT)?;
    let state = account(accounts, RESERVATION_STATE)?;
    let ordinal_seed = [ordinal];
    let bump = Pubkey::find_program_address(
        &[
            DEALER_SCENARIO_RESERVATION_STATE_PDA_DOMAIN_V1,
            checkpoint.key.as_ref(),
            ordinal_seed.as_slice(),
        ],
        program_id,
    )
    .1;
    let bump_seed = [bump];
    create_program_account(
        program_id,
        account(accounts, PAYER)?,
        state,
        account(accounts, SYSTEM)?,
        rent,
        DEALER_SCENARIO_RESERVATION_STATE_BYTES_V1,
        &[
            DEALER_SCENARIO_RESERVATION_STATE_PDA_DOMAIN_V1,
            checkpoint.key.as_ref(),
            ordinal_seed.as_slice(),
            bump_seed.as_slice(),
        ],
    )
}

fn create_receipt_account(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    ordinal: u8,
    action: DealerScenarioReservationActionV1,
    rent: &Rent,
) -> Result<(), ProgramError> {
    let checkpoint = account(accounts, CHECKPOINT)?;
    let receipt = account(accounts, RECEIPT)?;
    let ordinal_seed = [ordinal];
    let action_seed = [action as u8];
    let request = match action {
        DealerScenarioReservationActionV1::Reserve
        | DealerScenarioReservationActionV1::Rollback => {
            let effect_data = account(accounts, EFFECT_BODY)?
                .try_borrow_data()
                .map_err(|_| CustodySbfError::Release)?;
            DealerScenarioCustodyEffectV1::decode(&effect_data)
                .map_err(|_| CustodySbfError::Release)?
                .request_digest
        }
    };
    let bump = Pubkey::find_program_address(
        &[
            DEALER_SCENARIO_RESERVATION_RECEIPT_PDA_DOMAIN_V1,
            checkpoint.key.as_ref(),
            &request,
            action_seed.as_slice(),
            ordinal_seed.as_slice(),
        ],
        program_id,
    )
    .1;
    let bump_seed = [bump];
    create_program_account(
        program_id,
        account(accounts, PAYER)?,
        receipt,
        account(accounts, SYSTEM)?,
        rent,
        DEALER_SCENARIO_RESERVATION_RECEIPT_BYTES_V1,
        &[
            DEALER_SCENARIO_RESERVATION_RECEIPT_PDA_DOMAIN_V1,
            checkpoint.key.as_ref(),
            &request,
            action_seed.as_slice(),
            ordinal_seed.as_slice(),
            bump_seed.as_slice(),
        ],
    )
}

#[allow(clippy::too_many_arguments)]
fn create_program_account<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    system: &AccountInfo<'a>,
    rent: &Rent,
    bytes: usize,
    signer_seeds: &[&[u8]],
) -> Result<(), ProgramError> {
    require_vacant(destination)?;
    let lamports = rent.minimum_balance(bytes);
    invoke_signed(
        &create_account(
            payer.key,
            destination.key,
            lamports,
            u64::try_from(bytes).map_err(|_| CustodySbfError::Create)?,
            program_id,
        ),
        &[payer.clone(), destination.clone(), system.clone()],
        &[signer_seeds],
    )
    .map_err(|_| CustodySbfError::Create)?;
    if destination.owner != program_id
        || destination.data_len() != bytes
        || destination.lamports() != lamports
    {
        return Err(CustodySbfError::Create.into());
    }
    Ok(())
}

fn require_vacant(value: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if value.owner != &system_program::ID || value.data_len() != 0 || value.lamports() != 0 {
        return Err(CustodySbfError::Create.into());
    }
    Ok(())
}

fn write_exact(account: &AccountInfo<'_>, bytes: &[u8]) -> Result<(), ProgramError> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| CustodySbfError::Commit)?;
    if data.len() != bytes.len() {
        return Err(CustodySbfError::Commit.into());
    }
    data.copy_from_slice(bytes);
    Ok(())
}

#[inline(never)]
fn write_state_value(
    account: &AccountInfo<'_>,
    state: &DealerScenarioReservationStateV1,
) -> Result<[u8; 32], ProgramError> {
    let bytes = state.encode().map_err(|_| CustodySbfError::Commit)?;
    write_exact(account, &bytes)?;
    Ok(hash(&bytes).to_bytes())
}

#[inline(never)]
fn write_receipt_value(
    account: &AccountInfo<'_>,
    receipt: &DealerScenarioReservationReceiptV1,
) -> Result<[u8; 32], ProgramError> {
    let bytes = receipt.encode().map_err(|_| CustodySbfError::Commit)?;
    write_exact(account, &bytes)?;
    Ok(hash(&bytes).to_bytes())
}

#[inline(never)]
fn write_batch_value(
    account: &AccountInfo<'_>,
    batch: &DealerScenarioReservationBatchV1,
) -> Result<[u8; 32], ProgramError> {
    let bytes = batch.encode().map_err(|_| CustodySbfError::Commit)?;
    write_exact(account, &bytes)?;
    Ok(hash(&bytes).to_bytes())
}

#[inline(never)]
fn set_account_return_data(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Commit)?;
    set_return_data(&data);
    Ok(())
}

fn account_digest(account: &AccountInfo<'_>) -> Result<[u8; 32], ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Postcondition)?;
    Ok(hash(&data).to_bytes())
}

fn vacant_digest() -> [u8; 32] {
    hash(b"dclutch:dealer-vacant-account:v1").to_bytes()
}

fn require_reversible_staged_source(
    kind: DealerScenarioCustodyRequestKindV1,
    request: CustodyRequestV1,
) -> Result<(), ProgramError> {
    if kind != DealerScenarioCustodyRequestKindV1::Canonical
        || request.source_compartment == CompartmentV1::External
    {
        // External delegated debits consume allowance which Custody cannot
        // recreate during permissionless rollback. Deposit to an internal
        // TradingPrincipal vault first, then stage that canonical movement.
        return Err(CustodySbfError::Instruction.into());
    }
    Ok(())
}

fn has_duplicate_keys(accounts: &[AccountInfo<'_>]) -> bool {
    accounts.iter().enumerate().any(|(index, current)| {
        accounts
            .get(..index)
            .is_some_and(|prefix| prefix.iter().any(|prior| prior.key == current.key))
    })
}

/// The same census with one coordinate excused, for the sole slot the protocol
/// itself requires to repeat an identity the frame already carries.
fn has_duplicate_keys_except(accounts: &[AccountInfo<'_>], excused: usize) -> bool {
    accounts
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != excused)
        .any(|(index, current)| {
            accounts
                .get(..index)
                .is_some_and(|prefix| {
                    prefix
                        .iter()
                        .enumerate()
                        .any(|(prior_index, prior)| {
                            prior_index != excused && prior.key == current.key
                        })
                })
        })
}

// The transaction lock cap counts the program itself in addition to the frame.
const _: () = assert!(DEALER_SCENARIO_RESERVATION_ACCOUNT_COUNT_V1 + 1 < 64);

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_custody_contract::{CallerRoleV1, ContextV1};

    fn transfer() -> CustodyRequestV1 {
        CustodyRequestV1 {
            operation: OperationV1::Transfer,
            caller_role: CallerRoleV1::Trading,
            source_compartment: CompartmentV1::TradingPrincipal,
            destination_compartment: CompartmentV1::FeeVault,
            release_set: [1; 32],
            market: [2; 32],
            realm: [3; 32],
            context: [4; 32],
            caller_program: [5; 32],
            semantic: ContextV1 {
                candidate: [6; 32],
                source_owner: [0; 32],
                destination_owner: [0; 32],
                order: [7; 32],
                parent_request_digest: [8; 32],
                order_nonce: 9,
                generation: 10,
                page_index: 0,
                execution_index: 0,
                transfer_index: 0,
            },
            source: [11; 32],
            destination: [12; 32],
            source_vault_context: [13; 32],
            destination_vault_context: [14; 32],
            mint: [15; 32],
            token_program: [16; 32],
            payer: [0; 32],
            rent_refund: [0; 32],
            expected_revision: 1,
            resulting_revision: 2,
            amount: 50,
            rent_lamports: 0,
        }
    }

    #[test]
    fn reserve_and_rollback_preserve_original_compartment_semantics() {
        let original = transfer();
        assert!(
            require_reversible_staged_source(
                DealerScenarioCustodyRequestKindV1::Canonical,
                original
            )
            .is_ok()
        );
        let mut external = original;
        external.source_compartment = CompartmentV1::External;
        external.semantic.source_owner = [22; 32];
        assert!(
            require_reversible_staged_source(
                DealerScenarioCustodyRequestKindV1::Canonical,
                external
            )
            .is_err()
        );
        assert!(
            require_reversible_staged_source(
                DealerScenarioCustodyRequestKindV1::Delegated,
                original
            )
            .is_err()
        );
        let reserve = reserve_request(
            original,
            Pubkey::new_from_array([20; 32]),
            Pubkey::new_from_array([21; 32]),
        );
        assert_eq!(reserve.source_compartment, CompartmentV1::TradingPrincipal);
        assert_eq!(
            reserve.destination_compartment,
            CompartmentV1::RecoveryReserve
        );
        assert_eq!(reserve.amount, original.amount);
        assert!(reserve.validate().is_ok());
        let rollback = rollback_request(
            original,
            Pubkey::new_from_array([20; 32]),
            Pubkey::new_from_array([21; 32]),
        );
        assert_eq!(rollback.source_compartment, CompartmentV1::RecoveryReserve);
        assert_eq!(
            rollback.destination_compartment,
            CompartmentV1::TradingPrincipal
        );
        assert_eq!(rollback.destination, original.source);
        assert!(rollback.validate().is_ok());
        let activation = activate_request(
            original,
            Pubkey::new_from_array([20; 32]),
            Pubkey::new_from_array([21; 32]),
        );
        assert_eq!(
            activation.source_compartment,
            CompartmentV1::RecoveryReserve
        );
        assert_eq!(activation.destination_compartment, CompartmentV1::FeeVault);
        assert_eq!(activation.destination, original.destination);
        assert_eq!(activation.amount, original.amount);
        assert!(activation.validate().is_ok());

        let mut checkpoint = CheckpointFactsV1 {
            input: DealerScenarioCheckpointInputV1 {
                release_set: [1; 32],
                market: [2; 32],
                child_root: [3; 32],
                obligation: [4; 32],
                refund_beneficiary: [5; 32],
                request_digest: [6; 32],
                membership_manifest_digest: [7; 32],
                root_prestate_digest: [8; 32],
                claims_prestate_digest: [9; 32],
                obligation_prestate_digest: [10; 32],
                custody_prestate_digest: [11; 32],
                generation: 1,
                created_slot: 1,
                expires_at: 2,
            },
            digest: [12; 32],
            phase: DealerScenarioCheckpointPhaseV1::Reserved,
            reservation_count: 2,
            rollback_count: 0,
            reservation_receipt: [13; 32],
            effect_count: 2,
            effects_digest: [14; 32],
        };
        assert!(require_committed_checkpoint(&checkpoint, 2).is_err());
        checkpoint.phase = DealerScenarioCheckpointPhaseV1::Committed;
        assert!(require_committed_checkpoint(&checkpoint, 2).is_ok());
        checkpoint.rollback_count = 1;
        assert!(require_committed_checkpoint(&checkpoint, 2).is_err());
    }
}
