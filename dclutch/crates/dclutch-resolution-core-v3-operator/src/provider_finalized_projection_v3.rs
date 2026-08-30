//! Exact finalized projections for the three Resolution provider stages.
//!
//! This module does not construct, sign, or submit transactions.  It starts
//! from one already-frozen instruction, the exact writable prestates captured
//! before submission, and finalized return data/poststates.  Each stage has a
//! disjoint input and output type so a submit receipt cannot be accepted as an
//! execute or reclaim receipt.  The projections mirror the transition owners
//! in `dclutch-resolution-proof-sbf` and produce exact account bytes rather
//! than accepting a poststate digest supplied by a caller.

use dclutch_market_core_codec::{Action, CoreState, Phase, REQUEST_BYTES, Readiness, Request};
use dclutch_product_runtime_v2::ResultDomainV2;
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_resolution_codec::{
    PROVIDER_EXECUTION_REQUEST_BYTES_V3, PROVIDER_RECLAIM_REQUEST_BYTES_V3,
    PROVIDER_SUBMIT_REQUEST_BYTES_V3, PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3,
    PROVIDER_UPDATE_LIFECYCLE_BYTES_V3, PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3, ProviderCallerV3,
    ProviderExecutionReceiptV3, ProviderExecutionRequestV3, ProviderReclaimReceiptV3,
    ProviderReclaimRequestV3, ProviderSubmitReceiptV3, ProviderSubmitRequestV3,
    ProviderUpdateLifecycleV3, ProviderUpdateStatusV3, RESOLUTION_CERTIFICATE_BYTES_V2,
    RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3, ResolutionCertificateKindV2, ResolutionCertificateV2,
};
use dclutch_source_contract::{
    ContentId as SourceContentId, SourceMaterialV3, SourceResolutionPhaseV1,
    SourceResolutionRouteV1, SourceResolutionStateV2,
};
use solana_program::{
    hash::{hash, hashv},
    instruction::Instruction,
    pubkey::Pubkey,
    rent::Rent,
};
use solana_sdk_ids::system_program;

use crate::{Finality, ObservedAccount};

/// Core-caller provider execution has this frozen account width.
pub const PROVIDER_EXECUTE_ACCOUNT_COUNT_V3: usize = 47;
/// Provider submission has this frozen account width.
pub const PROVIDER_SUBMIT_ACCOUNT_COUNT_V3: usize = 38;
/// Provider reclaim has this frozen account width.
pub const PROVIDER_RECLAIM_ACCOUNT_COUNT_V3: usize = 18;

/// Domain used by the Resolution transition owner to bind provider evidence.
pub const PROVIDER_EVIDENCE_DOMAIN_V3: &[u8] = b"dclutch/pyth-provider-evidence/v3";

/// Stable refusal from an exact finalized provider projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderFinalizedProjectionErrorV3 {
    /// Instruction width, discriminator, account order, or privilege differed.
    Instruction,
    /// A pre-execution account was not the exact state admitted by the stage.
    Prestate,
    /// Return-data producer, width, type, or request join differed.
    ReturnData,
    /// A finalized account was not from the execution slot or differed exactly.
    Poststate,
    /// Source, lifecycle, certificate, or Market transition refused.
    Transition,
    /// Checked balance or time arithmetic overflowed.
    Arithmetic,
}

/// Exact canonical account state projected for one writable protocol account.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderExpectedAccountStateV3 {
    /// Account address.
    pub key: Pubkey,
    /// Expected owner after execution.
    pub owner: Pubkey,
    /// Expected lamports after execution, excluding transaction-fee accounting.
    pub lamports: u64,
    /// Expected executable bit.
    pub executable: bool,
    /// Exact expected data bytes.
    pub data: Vec<u8>,
}

/// Exact submit prestates and finalized poststates in frozen writable order.
#[derive(Clone, Copy)]
pub struct ProviderSubmitWritableAccountsV3<'a> {
    /// Submitter before provider rent/fee debit.
    pub submitter_before: &'a ObservedAccount,
    /// Vacant Receiver update before posting.
    pub update_before: &'a ObservedAccount,
    /// Prefunded vacant lifecycle before allocation.
    pub lifecycle_before: &'a ObservedAccount,
    /// Receiver treasury before provider fee credit.
    pub treasury_before: &'a ObservedAccount,
    /// Submitter after provider rent/fee debit.
    pub submitter_after: &'a ObservedAccount,
    /// Receiver update after posting.
    pub update_after: &'a ObservedAccount,
    /// Submitted lifecycle after allocation.
    pub lifecycle_after: &'a ObservedAccount,
    /// Receiver treasury after provider fee credit.
    pub treasury_after: &'a ObservedAccount,
}

/// Inputs for one finalized provider submission projection.
#[derive(Clone, Copy)]
pub struct ProviderSubmitFinalizedInputV3<'a> {
    /// Exact top-level Resolution instruction from the durable message.
    pub instruction: &'a Instruction,
    /// Program ID reported by finalized transaction return data.
    pub return_data_program: Pubkey,
    /// Exact finalized return-data bytes.
    pub return_data: &'a [u8],
    /// Finalized execution slot reported by `getTransaction`.
    pub finalized_slot: u64,
    /// Exact transaction fee debited from the submitter/fee payer before the
    /// Resolution instruction executed.
    pub transaction_fee_lamports: u64,
    /// Exact System transfer to the vacant lifecycle earlier in the same
    /// durable message.
    pub lifecycle_top_up_lamports: u64,
    /// Exact provider fee authenticated from the pinned Receiver Config before
    /// this durable message was signed.
    pub expected_provider_fee_lamports: u64,
    /// Exact Rent parameters used by the executed bank.
    pub rent: &'a Rent,
    /// Exact writable prestates and finalized poststates.
    pub writable: ProviderSubmitWritableAccountsV3<'a>,
}

/// Canonical finalized provider submission projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderSubmitProjectionV3 {
    /// Exact canonical instruction authenticated by this projection.
    pub expected_instruction: Instruction,
    /// Canonically decoded exact submit request.
    pub request: ProviderSubmitRequestV3,
    /// Canonically decoded and rejoined typed return data.
    pub receipt: ProviderSubmitReceiptV3,
    /// Exact poststates in instruction writable order: submitter, update,
    /// lifecycle, and Receiver treasury.
    pub expected_writable_poststates: [ProviderExpectedAccountStateV3; 4],
}

/// Exact execute prestates and finalized poststates in frozen writable order.
#[derive(Clone, Copy)]
pub struct ProviderExecuteWritableAccountsV3<'a> {
    /// Primary Source state before resolution.
    pub source_before: &'a ObservedAccount,
    /// Prefunded vacant certificate before allocation.
    pub certificate_before: &'a ObservedAccount,
    /// Open/Consumed Market before terminal admission.
    pub market_before: &'a ObservedAccount,
    /// Submitted provider lifecycle before consumption.
    pub lifecycle_before: &'a ObservedAccount,
    /// Resolved Source state after execution.
    pub source_after: &'a ObservedAccount,
    /// Terminal certificate after allocation.
    pub certificate_after: &'a ObservedAccount,
    /// Unchanged Open Market after the caller-only Core wrapper.
    pub market_after: &'a ObservedAccount,
    /// Consumed provider lifecycle after execution.
    pub lifecycle_after: &'a ObservedAccount,
}

/// Inputs for one finalized Core-caller provider execution projection.
#[derive(Clone, Copy)]
pub struct ProviderExecuteFinalizedInputV3<'a> {
    /// Exact top-level Core instruction from the durable message.
    pub instruction: &'a Instruction,
    /// Resolution child program ID reported by finalized return data.
    pub return_data_program: Pubkey,
    /// Exact finalized typed child return data.
    pub return_data: &'a [u8],
    /// Finalized execution/Clock slot.
    pub finalized_slot: u64,
    /// Exact positive Clock Unix time used by Source resolution.
    pub execution_unix_timestamp: i64,
    /// Exact Rent parameters used by the executed bank.
    pub rent: &'a Rent,
    /// Exact System transfer to the vacant certificate earlier in the same
    /// durable message.
    pub certificate_top_up_lamports: u64,
    /// Exact finalized SourceMaterial content bytes selected by the request.
    pub source_material: &'a ObservedAccount,
    /// Exact finalized ResultDomain content bytes selected by the request.
    pub result_domain: &'a ObservedAccount,
    /// Exact unchanged provider update read by Resolution.
    pub update: &'a ObservedAccount,
    /// Exact mutated prestates plus the read-only Market observation and all
    /// corresponding finalized poststates.
    pub writable: ProviderExecuteWritableAccountsV3<'a>,
}

/// Canonical finalized provider execution projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderExecuteProjectionV3 {
    /// Exact canonical instruction authenticated by this projection.
    pub expected_instruction: Instruction,
    /// Canonically decoded exact Core request.
    pub core_request: Request,
    /// Canonically decoded exact provider request.
    pub request: ProviderExecutionRequestV3,
    /// Canonically decoded and rejoined typed Resolution return data.
    pub receipt: ProviderExecutionReceiptV3,
    /// Exact observed poststates: Source, certificate, unchanged Market, and
    /// lifecycle. Only Source, certificate, and lifecycle are writable.
    pub expected_writable_poststates: [ProviderExpectedAccountStateV3; 4],
}

/// Exact reclaim prestates and finalized poststates in frozen writable order.
#[derive(Clone, Copy)]
pub struct ProviderReclaimWritableAccountsV3<'a> {
    /// Consumed lifecycle before closing.
    pub lifecycle_before: &'a ObservedAccount,
    /// Receiver update before closing.
    pub update_before: &'a ObservedAccount,
    /// Vacant update-authority PDA before the Receiver credit.
    pub authority_before: &'a ObservedAccount,
    /// Immutable refund recipient before credits.
    pub refund_before: &'a ObservedAccount,
    /// Closed lifecycle after reclaim.
    pub lifecycle_after: &'a ObservedAccount,
    /// Closed Receiver update after reclaim.
    pub update_after: &'a ObservedAccount,
    /// Vacant update-authority PDA after forwarding update rent.
    pub authority_after: &'a ObservedAccount,
    /// Refund recipient after update-rent and lifecycle-rent credits.
    pub refund_after: &'a ObservedAccount,
}

/// Inputs for one finalized permissionless provider reclaim projection.
#[derive(Clone, Copy)]
pub struct ProviderReclaimFinalizedInputV3<'a> {
    /// Exact top-level Resolution instruction from the durable message.
    pub instruction: &'a Instruction,
    /// Program ID reported by finalized transaction return data.
    pub return_data_program: Pubkey,
    /// Exact finalized return-data bytes.
    pub return_data: &'a [u8],
    /// Finalized execution slot.
    pub finalized_slot: u64,
    /// Exact positive Clock Unix time used by the reclaim gate.
    pub execution_unix_timestamp: i64,
    /// Exact Rent parameters used by the executed bank.
    pub rent: &'a Rent,
    /// Immutable terminal certificate authenticated by reclaim.
    pub certificate: &'a ObservedAccount,
    /// Exact writable prestates and finalized poststates.
    pub writable: ProviderReclaimWritableAccountsV3<'a>,
}

/// Canonical finalized permissionless reclaim projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderReclaimProjectionV3 {
    /// Exact canonical instruction authenticated by this projection.
    pub expected_instruction: Instruction,
    /// Canonically decoded exact reclaim request.
    pub request: ProviderReclaimRequestV3,
    /// Canonically decoded and rejoined typed return data.
    pub receipt: ProviderReclaimReceiptV3,
    /// Exact poststates in instruction writable order: lifecycle, update,
    /// update-authority PDA, and refund recipient.
    pub expected_writable_poststates: [ProviderExpectedAccountStateV3; 4],
}

/// Project and authenticate one finalized provider submission.
pub fn project_finalized_provider_submit_v3(
    input: ProviderSubmitFinalizedInputV3<'_>,
) -> Result<ProviderSubmitProjectionV3, ProviderFinalizedProjectionErrorV3> {
    let instruction = input.instruction;
    require_frame(
        instruction,
        PROVIDER_SUBMIT_ACCOUNT_COUNT_V3,
        14,
        &[0, 1],
        &[0, 1, 2, 34],
    )?;
    let prefix = instruction
        .data
        .get(..PROVIDER_SUBMIT_REQUEST_BYTES_V3)
        .ok_or(ProviderFinalizedProjectionErrorV3::Instruction)?;
    let body = instruction
        .data
        .get(PROVIDER_SUBMIT_REQUEST_BYTES_V3..)
        .filter(|body| !body.is_empty())
        .ok_or(ProviderFinalizedProjectionErrorV3::Instruction)?;
    let request = ProviderSubmitRequestV3::decode(prefix)
        .map_err(|_| ProviderFinalizedProjectionErrorV3::Instruction)?;
    let expected_authority = Pubkey::find_program_address(
        &[
            PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3,
            &request.market,
            &request.source_state,
            &request.update_account,
        ],
        &instruction.program_id,
    )
    .0;
    let expected_treasury =
        Pubkey::find_program_address(&[b"treasury", &[0]], &key_at(instruction, 27)?).0;
    if request
        .to_bytes()
        .map_err(|_| ProviderFinalizedProjectionErrorV3::Instruction)?
        .as_slice()
        != prefix
        || request.post_body_digest != hash(body).to_bytes()
        || key_at(instruction, 0)?.to_bytes() != request.provider_submitter
        || key_at(instruction, 1)?.to_bytes() != request.update_account
        || key_at(instruction, 2)?.to_bytes() != request.lifecycle
        || key_at(instruction, 3)? != expected_authority
        || key_at(instruction, 4)?.to_bytes() != request.refund_recipient
        || key_at(instruction, 5)?.to_bytes() != request.market
        || key_at(instruction, 8)?.to_bytes() != request.registry_program
        || key_at(instruction, 16)?.to_bytes() != request.source_state
        || key_at(instruction, 32)?.to_bytes() != request.encoded_vaa
        || key_at(instruction, 34)? != expected_treasury
    {
        return Err(ProviderFinalizedProjectionErrorV3::Instruction);
    }
    let w = input.writable;
    require_pre_keys(
        &[
            w.submitter_before,
            w.update_before,
            w.lifecycle_before,
            w.treasury_before,
        ],
        &[
            key_at(instruction, 0)?,
            key_at(instruction, 1)?,
            key_at(instruction, 2)?,
            key_at(instruction, 34)?,
        ],
        input.finalized_slot,
    )?;
    require_post_keys(
        &[
            w.submitter_after,
            w.update_after,
            w.lifecycle_after,
            w.treasury_after,
        ],
        &[
            key_at(instruction, 0)?,
            key_at(instruction, 1)?,
            key_at(instruction, 2)?,
            key_at(instruction, 34)?,
        ],
        input.finalized_slot,
    )?;
    let minimum_lifecycle = input
        .rent
        .minimum_balance(PROVIDER_UPDATE_LIFECYCLE_BYTES_V3);
    let lifecycle_after_top_up = w
        .lifecycle_before
        .lamports
        .checked_add(input.lifecycle_top_up_lamports)
        .ok_or(ProviderFinalizedProjectionErrorV3::Arithmetic)?;
    if !vacant(w.update_before)
        || w.update_before.lamports != 0
        || !vacant(w.lifecycle_before)
        || lifecycle_after_top_up != minimum_lifecycle
    {
        return Err(ProviderFinalizedProjectionErrorV3::Prestate);
    }
    let receipt = ProviderSubmitReceiptV3::decode(input.return_data)
        .map_err(|_| ProviderFinalizedProjectionErrorV3::ReturnData)?;
    if input.return_data_program != instruction.program_id
        || receipt
            .to_bytes()
            .map_err(|_| ProviderFinalizedProjectionErrorV3::ReturnData)?
            .as_slice()
            != input.return_data
        || receipt.request_digest != hash(prefix).to_bytes()
        || receipt.lifecycle != request.lifecycle
        || receipt.update_account != request.update_account
        || receipt.update_digest != hash(&w.update_after.data).to_bytes()
        || receipt.provider_submitter != request.provider_submitter
        || receipt.update_authority != key_at(instruction, 3)?.to_bytes()
        || receipt.refund_recipient != request.refund_recipient
        || receipt.provider_release != request.provider_release
        || receipt.post_body_digest != request.post_body_digest
        || receipt.market != request.market
        || receipt.generation != request.generation
        || receipt.posted_slot == 0
        || receipt.posted_slot > input.finalized_slot
        || receipt.publish_time <= 0
        || request.reclaim_after_unix_seconds < receipt.publish_time
        || receipt.update_rent_lamports != input.rent.minimum_balance(w.update_after.data.len())
        || receipt.provider_fee_lamports != input.expected_provider_fee_lamports
        || w.update_after.owner != key_at(instruction, 27)?
        || w.update_after.lamports != receipt.update_rent_lamports
        || w.update_after.executable
    {
        return Err(ProviderFinalizedProjectionErrorV3::ReturnData);
    }
    let (expected_lifecycle, bump) = Pubkey::find_program_address(
        &[
            PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3,
            key_at(instruction, 1)?.as_ref(),
        ],
        &instruction.program_id,
    );
    if expected_lifecycle.to_bytes() != request.lifecycle {
        return Err(ProviderFinalizedProjectionErrorV3::Instruction);
    }
    let lifecycle = ProviderUpdateLifecycleV3::submitted(
        request,
        bump,
        receipt.update_authority,
        key_at(instruction, 8)?.to_bytes(),
        receipt.update_digest,
        receipt.publish_time,
        receipt.posted_slot,
        receipt.update_rent_lamports,
        receipt.provider_fee_lamports,
    )
    .map_err(|_| ProviderFinalizedProjectionErrorV3::Transition)?;
    let lifecycle_data = lifecycle
        .to_bytes()
        .map_err(|_| ProviderFinalizedProjectionErrorV3::Transition)?
        .to_vec();
    let expected_submitter_lamports = w
        .submitter_before
        .lamports
        .checked_sub(input.transaction_fee_lamports)
        .and_then(|value| value.checked_sub(input.lifecycle_top_up_lamports))
        .and_then(|value| value.checked_sub(receipt.update_rent_lamports))
        .and_then(|value| value.checked_sub(receipt.provider_fee_lamports))
        .ok_or(ProviderFinalizedProjectionErrorV3::Arithmetic)?;
    let expected_treasury_lamports = w
        .treasury_before
        .lamports
        .checked_add(receipt.provider_fee_lamports)
        .ok_or(ProviderFinalizedProjectionErrorV3::Arithmetic)?;
    let expected = [
        unchanged_with_lamports(w.submitter_before, expected_submitter_lamports),
        state(
            w.update_after.key,
            key_at(instruction, 27)?,
            receipt.update_rent_lamports,
            false,
            w.update_after.data.clone(),
        ),
        state(
            w.lifecycle_after.key,
            instruction.program_id,
            lifecycle_after_top_up,
            false,
            lifecycle_data,
        ),
        unchanged_with_lamports(w.treasury_before, expected_treasury_lamports),
    ];
    require_expected_posts(
        &[
            w.submitter_after,
            w.update_after,
            w.lifecycle_after,
            w.treasury_after,
        ],
        &expected,
    )?;
    Ok(ProviderSubmitProjectionV3 {
        expected_instruction: instruction.clone(),
        request,
        receipt,
        expected_writable_poststates: expected,
    })
}

/// Project and authenticate one finalized Core-caller provider execution.
pub fn project_finalized_provider_execute_v3(
    input: ProviderExecuteFinalizedInputV3<'_>,
) -> Result<ProviderExecuteProjectionV3, ProviderFinalizedProjectionErrorV3> {
    let instruction = input.instruction;
    require_frame(
        instruction,
        PROVIDER_EXECUTE_ACCOUNT_COUNT_V3,
        11,
        &[1],
        &[2, 3, 37],
    )?;
    let provider_start = REQUEST_BYTES;
    let provider_end = provider_start
        .checked_add(PROVIDER_EXECUTION_REQUEST_BYTES_V3)
        .ok_or(ProviderFinalizedProjectionErrorV3::Arithmetic)?;
    let core_bytes = instruction
        .data
        .get(..provider_start)
        .ok_or(ProviderFinalizedProjectionErrorV3::Instruction)?;
    let provider_bytes = instruction
        .data
        .get(provider_start..provider_end)
        .ok_or(ProviderFinalizedProjectionErrorV3::Instruction)?;
    let body = instruction
        .data
        .get(provider_end..)
        .filter(|body| !body.is_empty())
        .ok_or(ProviderFinalizedProjectionErrorV3::Instruction)?;
    let core_request =
        Request::decode(core_bytes).map_err(|_| ProviderFinalizedProjectionErrorV3::Instruction)?;
    let request = ProviderExecutionRequestV3::decode(provider_bytes)
        .map_err(|_| ProviderFinalizedProjectionErrorV3::Instruction)?;
    let authority = CallerAuthoritySeedsV1::from_bytes(
        request.release_set,
        request.market,
        ExecutionRoleV1::Core,
        request.source_state,
        request.parent_request_digest,
    )
    .map_err(|_| ProviderFinalizedProjectionErrorV3::Instruction)?;
    if core_request.action != Action::ExecuteProvider
        || core_request
            .encode()
            .map_err(|_| ProviderFinalizedProjectionErrorV3::Instruction)?
            .as_slice()
            != core_bytes
        || request
            .to_bytes()
            .map_err(|_| ProviderFinalizedProjectionErrorV3::Instruction)?
            .as_slice()
            != provider_bytes
        || request.caller != ProviderCallerV3::Core
        || request.market != core_request.market.to_bytes()
        || request.generation != core_request.generation
        || request.parent_request_digest != hash(core_bytes).to_bytes()
        || request.post_params_body_digest != hash(body).to_bytes()
        || request.caller_program != instruction.program_id.to_bytes()
        || key_at(instruction, 0)?
            != Pubkey::find_program_address(&authority.as_slices(), &instruction.program_id).0
        || key_at(instruction, 1)?.to_bytes() != request.resolver
        || key_at(instruction, 2)?.to_bytes() != request.source_state
        || key_at(instruction, 3)?.to_bytes() != request.certificate_account
        || key_at(instruction, 4)?.to_bytes() != request.market
        || key_at(instruction, 37)?.to_bytes()
            != Pubkey::find_program_address(
                &[
                    PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3,
                    &request.update_account,
                ],
                &key_at(instruction, 15)?,
            )
            .0
            .to_bytes()
        || key_at(instruction, 38)?.to_bytes() != request.update_account
    {
        return Err(ProviderFinalizedProjectionErrorV3::Instruction);
    }
    let w = input.writable;
    require_pre_keys(
        &[
            w.source_before,
            w.certificate_before,
            w.market_before,
            w.lifecycle_before,
        ],
        &[
            key_at(instruction, 2)?,
            key_at(instruction, 3)?,
            key_at(instruction, 4)?,
            key_at(instruction, 37)?,
        ],
        input.finalized_slot,
    )?;
    require_post_keys(
        &[
            w.source_after,
            w.certificate_after,
            w.market_after,
            w.lifecycle_after,
        ],
        &[
            key_at(instruction, 2)?,
            key_at(instruction, 3)?,
            key_at(instruction, 4)?,
            key_at(instruction, 37)?,
        ],
        input.finalized_slot,
    )?;
    let certificate_after_top_up = w
        .certificate_before
        .lamports
        .checked_add(input.certificate_top_up_lamports)
        .ok_or(ProviderFinalizedProjectionErrorV3::Arithmetic)?;
    if input.execution_unix_timestamp <= 0
        || input.update.key != key_at(instruction, 38)?
        || input.update.owner != key_at(instruction, 39)?
        || hash(&input.update.data).to_bytes() != request.expected_update_digest
        || hash(&input.source_material.data).to_bytes() != request.source_material
        || hash(&input.result_domain.data).to_bytes() != request.result_domain
        || !vacant(w.certificate_before)
        || certificate_after_top_up != input.rent.minimum_balance(RESOLUTION_CERTIFICATE_BYTES_V2)
    {
        return Err(ProviderFinalizedProjectionErrorV3::Prestate);
    }
    let resolution_program = key_at(instruction, 15)?;
    let receipt = ProviderExecutionReceiptV3::decode(input.return_data)
        .map_err(|_| ProviderFinalizedProjectionErrorV3::ReturnData)?;
    let expected_evidence = hashv(&[
        PROVIDER_EVIDENCE_DOMAIN_V3,
        &[0],
        &request.source_spec,
        &request.provider_release,
        &request.update_account,
        &request.expected_update_digest,
        &request.post_params_body_digest,
    ])
    .to_bytes();
    if input.return_data_program != resolution_program
        || receipt
            .to_bytes()
            .map_err(|_| ProviderFinalizedProjectionErrorV3::ReturnData)?
            .as_slice()
            != input.return_data
        || receipt.caller != request.caller
        || receipt.generation != request.generation
        || receipt.terminal_sequence != request.terminal_sequence
        || receipt.request_digest != hash(provider_bytes).to_bytes()
        || receipt.provider_evidence != expected_evidence
        || receipt.update_digest != request.expected_update_digest
        || receipt.post_params_body_digest != request.post_params_body_digest
        || receipt.market != request.market
        || receipt.source_state != request.source_state
        || receipt.certificate_account != request.certificate_account
        || receipt.source_material != request.source_material
        || receipt.product_record != request.product_record
        || receipt.result_domain != request.result_domain
        || receipt.provider_release != request.provider_release
        || receipt.update_account != request.update_account
        || receipt.provider_submitter != request.provider_submitter
        || receipt.resolver != request.resolver
        || receipt.caller_program != request.caller_program
        || receipt.release_set != request.release_set
        || receipt.capability_program_set != [0; 32]
        || receipt.selected_capability_program != [0; 32]
        || receipt.result_denominator != 1
        || receipt.consumed_slot != input.finalized_slot
        || receipt.posted_slot > receipt.consumed_slot
    {
        return Err(ProviderFinalizedProjectionErrorV3::ReturnData);
    }
    let mut source = SourceResolutionStateV2::decode(&w.source_before.data)
        .map_err(|_| ProviderFinalizedProjectionErrorV3::Prestate)?;
    let material = SourceMaterialV3::decode(&input.source_material.data)
        .map_err(|_| ProviderFinalizedProjectionErrorV3::Prestate)?;
    let domain = ResultDomainV2::decode(&input.result_domain.data)
        .map_err(|_| ProviderFinalizedProjectionErrorV3::Prestate)?;
    let source_seeds = source.pda_seeds();
    let source_bump = [source_seeds.bump()];
    let expected_source = Pubkey::create_program_address(
        &[
            source_seeds.domain(),
            &source_seeds.market(),
            &source_seeds.generation_le(),
            &source_bump,
        ],
        &resolution_program,
    )
    .map_err(|_| ProviderFinalizedProjectionErrorV3::Prestate)?;
    if w.source_before.owner != resolution_program
        || w.source_before.executable
        || w.source_before.key != expected_source
        || source.phase() != SourceResolutionPhaseV1::Primary
        || source.market() != request.market
        || source.generation() != request.generation
        || source.material_id().to_bytes() != request.source_material
        || material.product_record_digest().to_bytes() != request.product_record
    {
        return Err(ProviderFinalizedProjectionErrorV3::Prestate);
    }
    let decision = source
        .resolve_primary_from_authenticated_domain(
            SourceContentId::new(request.source_material)
                .map_err(|_| ProviderFinalizedProjectionErrorV3::Transition)?,
            material,
            material.product_record_digest(),
            domain,
            SourceContentId::new(expected_evidence)
                .map_err(|_| ProviderFinalizedProjectionErrorV3::Transition)?,
            receipt.result_numerator,
            1,
            request.generation,
            input.execution_unix_timestamp,
            request.terminal_sequence,
        )
        .map_err(|_| ProviderFinalizedProjectionErrorV3::Transition)?;
    if decision.route() != SourceResolutionRouteV1::Primary
        || decision.selector() != receipt.selector
        || decision.outcome_count() != receipt.outcome_count
    {
        return Err(ProviderFinalizedProjectionErrorV3::Transition);
    }
    let certificate = ResolutionCertificateV2 {
        kind: ResolutionCertificateKindV2::ResolutionSuccess,
        market: request.market,
        route: request.provider_release,
        source_material: request.source_material,
        product_record_digest: request.product_record,
        provider_evidence: expected_evidence,
        funding_allocation: [0; 32],
        receipt_account: request.certificate_account,
        generation: request.generation,
        attempt_index: 0,
        schedule_index: 0,
        selector: receipt.selector,
        work_paid: 0,
        funding_remaining: 0,
        result_numerator: receipt.result_numerator,
        result_denominator: 1,
        observed_at: u64::try_from(receipt.publish_time)
            .map_err(|_| ProviderFinalizedProjectionErrorV3::Arithmetic)?,
    };
    certificate
        .validate_terminal_product(request.product_record, receipt.outcome_count)
        .map_err(|_| ProviderFinalizedProjectionErrorV3::Transition)?;
    let certificate_data = certificate
        .to_bytes()
        .map_err(|_| ProviderFinalizedProjectionErrorV3::Transition)?
        .to_vec();
    let (expected_certificate, _) = Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            &request.source_state,
            &[1],
            &request.terminal_sequence.to_le_bytes(),
        ],
        &resolution_program,
    );
    if expected_certificate.to_bytes() != request.certificate_account {
        return Err(ProviderFinalizedProjectionErrorV3::Instruction);
    }
    let mut lifecycle = ProviderUpdateLifecycleV3::decode(&w.lifecycle_before.data)
        .map_err(|_| ProviderFinalizedProjectionErrorV3::Prestate)?;
    let expected_lifecycle_bump = Pubkey::find_program_address(
        &[
            PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3,
            &request.update_account,
        ],
        &resolution_program,
    )
    .1;
    let expected_authority = Pubkey::find_program_address(
        &[
            PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3,
            &request.market,
            &request.source_state,
            &request.update_account,
        ],
        &resolution_program,
    )
    .0;
    if w.lifecycle_before.owner != resolution_program
        || w.lifecycle_before.executable
        || !input
            .rent
            .is_exempt(w.lifecycle_before.lamports, w.lifecycle_before.data.len())
        || lifecycle.status != ProviderUpdateStatusV3::Submitted
        || lifecycle.bump != expected_lifecycle_bump
        || lifecycle.generation != request.generation
        || lifecycle.market != request.market
        || lifecycle.source_state != request.source_state
        || lifecycle.source_material != request.source_material
        || lifecycle.provider_release != request.provider_release
        || lifecycle.update_account != request.update_account
        || lifecycle.update_digest != request.expected_update_digest
        || lifecycle.post_body_digest != request.post_params_body_digest
        || lifecycle.provider_submitter != request.provider_submitter
        || lifecycle.update_authority != expected_authority.to_bytes()
        || lifecycle.release_set != request.release_set
        || lifecycle.registry_program != key_at(instruction, 7)?.to_bytes()
        || lifecycle.publish_time != receipt.publish_time
        || lifecycle.posted_slot != receipt.posted_slot
    {
        return Err(ProviderFinalizedProjectionErrorV3::Prestate);
    }
    lifecycle
        .consume(
            request.terminal_sequence,
            expected_evidence,
            request.certificate_account,
        )
        .map_err(|_| ProviderFinalizedProjectionErrorV3::Transition)?;
    let lifecycle_data = lifecycle
        .to_bytes()
        .map_err(|_| ProviderFinalizedProjectionErrorV3::Transition)?
        .to_vec();
    let market = CoreState::decode(&w.market_before.data)
        .map_err(|_| ProviderFinalizedProjectionErrorV3::Prestate)?;
    if w.market_before.owner != instruction.program_id
        || w.market_before.executable
        || market.phase != Phase::Open
        || market.readiness != Readiness::Consumed
        || market.identity.market_id.to_bytes() != request.market
        || market.identity.generation != request.generation
        || market.identity.product_record.to_bytes() != request.product_record
        || market.identity.resolution_policy.to_bytes() != request.source_material
        || market.identity.selected_release_set.to_bytes() != request.release_set
    {
        return Err(ProviderFinalizedProjectionErrorV3::Prestate);
    }
    let expected = [
        state(
            w.source_after.key,
            resolution_program,
            w.source_before.lamports,
            false,
            source.to_bytes().to_vec(),
        ),
        state(
            w.certificate_after.key,
            resolution_program,
            certificate_after_top_up,
            false,
            certificate_data,
        ),
        state(
            w.market_after.key,
            instruction.program_id,
            w.market_before.lamports,
            false,
            w.market_before.data.clone(),
        ),
        state(
            w.lifecycle_after.key,
            resolution_program,
            w.lifecycle_before.lamports,
            false,
            lifecycle_data,
        ),
    ];
    require_expected_posts(
        &[
            w.source_after,
            w.certificate_after,
            w.market_after,
            w.lifecycle_after,
        ],
        &expected,
    )?;
    Ok(ProviderExecuteProjectionV3 {
        expected_instruction: instruction.clone(),
        core_request,
        request,
        receipt,
        expected_writable_poststates: expected,
    })
}

/// Project and authenticate one finalized permissionless provider reclaim.
pub fn project_finalized_provider_reclaim_v3(
    input: ProviderReclaimFinalizedInputV3<'_>,
) -> Result<ProviderReclaimProjectionV3, ProviderFinalizedProjectionErrorV3> {
    let instruction = input.instruction;
    require_frame(
        instruction,
        PROVIDER_RECLAIM_ACCOUNT_COUNT_V3,
        9,
        &[0],
        &[1, 2, 3, 4],
    )?;
    if instruction.data.len() != PROVIDER_RECLAIM_REQUEST_BYTES_V3 {
        return Err(ProviderFinalizedProjectionErrorV3::Instruction);
    }
    let request = ProviderReclaimRequestV3::decode(&instruction.data)
        .map_err(|_| ProviderFinalizedProjectionErrorV3::Instruction)?;
    if request
        .to_bytes()
        .map_err(|_| ProviderFinalizedProjectionErrorV3::Instruction)?
        .as_slice()
        != instruction.data
        || key_at(instruction, 0)?.to_bytes() != request.resolver
        || key_at(instruction, 1)?.to_bytes() != request.lifecycle
        || key_at(instruction, 2)?.to_bytes() != request.update_account
        || key_at(instruction, 4)?.to_bytes() != request.refund_recipient
        || key_at(instruction, 5)?.to_bytes() != request.certificate
    {
        return Err(ProviderFinalizedProjectionErrorV3::Instruction);
    }
    let w = input.writable;
    require_pre_keys(
        &[
            w.lifecycle_before,
            w.update_before,
            w.authority_before,
            w.refund_before,
        ],
        &[
            key_at(instruction, 1)?,
            key_at(instruction, 2)?,
            key_at(instruction, 3)?,
            key_at(instruction, 4)?,
        ],
        input.finalized_slot,
    )?;
    require_post_keys(
        &[
            w.lifecycle_after,
            w.update_after,
            w.authority_after,
            w.refund_after,
        ],
        &[
            key_at(instruction, 1)?,
            key_at(instruction, 2)?,
            key_at(instruction, 3)?,
            key_at(instruction, 4)?,
        ],
        input.finalized_slot,
    )?;
    let lifecycle = ProviderUpdateLifecycleV3::decode(&w.lifecycle_before.data)
        .map_err(|_| ProviderFinalizedProjectionErrorV3::Prestate)?;
    let (expected_lifecycle, expected_lifecycle_bump) = Pubkey::find_program_address(
        &[
            PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3,
            key_at(instruction, 2)?.as_ref(),
        ],
        &instruction.program_id,
    );
    let expected_authority = Pubkey::find_program_address(
        &[
            PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3,
            &request.market,
            &request.source_state,
            &request.update_account,
        ],
        &instruction.program_id,
    )
    .0;
    if input.execution_unix_timestamp < lifecycle.reclaim_after_unix_seconds
        || w.lifecycle_before.owner != instruction.program_id
        || w.lifecycle_before.executable
        || !input
            .rent
            .is_exempt(w.lifecycle_before.lamports, w.lifecycle_before.data.len())
        || w.lifecycle_before.key != expected_lifecycle
        || lifecycle.status != ProviderUpdateStatusV3::Consumed
        || lifecycle.bump != expected_lifecycle_bump
        || lifecycle.generation != request.generation
        || lifecycle.terminal_sequence != request.terminal_sequence
        || lifecycle.market != request.market
        || lifecycle.source_state != request.source_state
        || lifecycle.certificate != request.certificate
        || lifecycle.update_account != request.update_account
        || lifecycle.refund_recipient != request.refund_recipient
        || lifecycle.release_set != request.release_set
        || lifecycle.registry_program != key_at(instruction, 7)?.to_bytes()
        || lifecycle.update_authority != expected_authority.to_bytes()
        || key_at(instruction, 3)? != expected_authority
        || w.update_before.owner != key_at(instruction, 13)?
        || w.update_before.executable
        || w.update_before.lamports != lifecycle.update_rent_lamports
        || hash(&w.update_before.data).to_bytes() != lifecycle.update_digest
        || !vacant(w.authority_before)
        || w.authority_before.lamports != 0
    {
        return Err(ProviderFinalizedProjectionErrorV3::Prestate);
    }
    let certificate = ResolutionCertificateV2::decode(&input.certificate.data)
        .map_err(|_| ProviderFinalizedProjectionErrorV3::Prestate)?;
    if input.certificate.key != key_at(instruction, 5)?
        || input.certificate.owner != instruction.program_id
        || input.certificate.executable
        || certificate.market != lifecycle.market
        || certificate.source_material != lifecycle.source_material
        || certificate.provider_evidence != lifecycle.provider_evidence
        || certificate.receipt_account != lifecycle.certificate
        || certificate.generation != lifecycle.generation
    {
        return Err(ProviderFinalizedProjectionErrorV3::Prestate);
    }
    let receipt = ProviderReclaimReceiptV3::decode(input.return_data)
        .map_err(|_| ProviderFinalizedProjectionErrorV3::ReturnData)?;
    if input.return_data_program != instruction.program_id
        || receipt
            .to_bytes()
            .map_err(|_| ProviderFinalizedProjectionErrorV3::ReturnData)?
            .as_slice()
            != input.return_data
        || receipt.request_digest != hash(&instruction.data).to_bytes()
        || receipt.lifecycle != request.lifecycle
        || receipt.update_account != request.update_account
        || receipt.certificate != request.certificate
        || receipt.resolver != request.resolver
        || receipt.refund_recipient != request.refund_recipient
        || receipt.update_digest != lifecycle.update_digest
        || receipt.provider_evidence != lifecycle.provider_evidence
        || receipt.generation != request.generation
        || receipt.terminal_sequence != request.terminal_sequence
        || receipt.refunded_lamports != lifecycle.update_rent_lamports
    {
        return Err(ProviderFinalizedProjectionErrorV3::ReturnData);
    }
    let total_refund = lifecycle
        .update_rent_lamports
        .checked_add(w.lifecycle_before.lamports)
        .ok_or(ProviderFinalizedProjectionErrorV3::Arithmetic)?;
    let refund_lamports = w
        .refund_before
        .lamports
        .checked_add(total_refund)
        .ok_or(ProviderFinalizedProjectionErrorV3::Arithmetic)?;
    let expected = [
        state(
            w.lifecycle_after.key,
            system_program::ID,
            0,
            false,
            Vec::new(),
        ),
        state(w.update_after.key, system_program::ID, 0, false, Vec::new()),
        state(
            w.authority_after.key,
            system_program::ID,
            0,
            false,
            Vec::new(),
        ),
        unchanged_with_lamports(w.refund_before, refund_lamports),
    ];
    require_expected_posts(
        &[
            w.lifecycle_after,
            w.update_after,
            w.authority_after,
            w.refund_after,
        ],
        &expected,
    )?;
    Ok(ProviderReclaimProjectionV3 {
        expected_instruction: instruction.clone(),
        request,
        receipt,
        expected_writable_poststates: expected,
    })
}

fn require_frame(
    instruction: &Instruction,
    count: usize,
    program_index: usize,
    signer_indices: &[usize],
    writable_indices: &[usize],
) -> Result<(), ProviderFinalizedProjectionErrorV3> {
    if instruction.accounts.len() != count
        || instruction.program_id != key_at(instruction, program_index)?
        || instruction
            .accounts
            .iter()
            .enumerate()
            .any(|(index, account)| {
                account.is_signer != signer_indices.contains(&index)
                    || account.is_writable != writable_indices.contains(&index)
                    || instruction
                        .accounts
                        .iter()
                        .skip(index + 1)
                        .any(|other| other.pubkey == account.pubkey)
            })
    {
        Err(ProviderFinalizedProjectionErrorV3::Instruction)
    } else {
        Ok(())
    }
}

fn key_at(
    instruction: &Instruction,
    index: usize,
) -> Result<Pubkey, ProviderFinalizedProjectionErrorV3> {
    instruction
        .accounts
        .get(index)
        .map(|account| account.pubkey)
        .ok_or(ProviderFinalizedProjectionErrorV3::Instruction)
}

fn require_pre_keys(
    accounts: &[&ObservedAccount],
    keys: &[Pubkey],
    finalized_slot: u64,
) -> Result<(), ProviderFinalizedProjectionErrorV3> {
    let observation = accounts
        .first()
        .ok_or(ProviderFinalizedProjectionErrorV3::Prestate)?
        .observation;
    if accounts.len() != keys.len()
        || observation.finality != Finality::Finalized
        || observation.slot > finalized_slot
        || accounts
            .iter()
            .zip(keys)
            .any(|(account, key)| account.key != *key || account.observation != observation)
    {
        Err(ProviderFinalizedProjectionErrorV3::Prestate)
    } else {
        Ok(())
    }
}

fn require_post_keys(
    accounts: &[&ObservedAccount],
    keys: &[Pubkey],
    finalized_slot: u64,
) -> Result<(), ProviderFinalizedProjectionErrorV3> {
    let observation = accounts
        .first()
        .ok_or(ProviderFinalizedProjectionErrorV3::Poststate)?
        .observation;
    if accounts.len() != keys.len()
        || observation.finality != Finality::Finalized
        || observation.slot < finalized_slot
        || accounts
            .iter()
            .zip(keys)
            .any(|(account, key)| account.key != *key || account.observation != observation)
    {
        Err(ProviderFinalizedProjectionErrorV3::Poststate)
    } else {
        Ok(())
    }
}

fn require_expected_posts(
    observed: &[&ObservedAccount],
    expected: &[ProviderExpectedAccountStateV3],
) -> Result<(), ProviderFinalizedProjectionErrorV3> {
    if observed.len() != expected.len()
        || observed.iter().zip(expected).any(|(actual, expected)| {
            actual.key != expected.key
                || actual.owner != expected.owner
                || actual.lamports != expected.lamports
                || actual.executable != expected.executable
                || actual.data != expected.data
        })
    {
        Err(ProviderFinalizedProjectionErrorV3::Poststate)
    } else {
        Ok(())
    }
}

fn vacant(account: &ObservedAccount) -> bool {
    account.owner == system_program::ID && !account.executable && account.data.is_empty()
}

fn unchanged_with_lamports(
    account: &ObservedAccount,
    lamports: u64,
) -> ProviderExpectedAccountStateV3 {
    state(
        account.key,
        account.owner,
        lamports,
        account.executable,
        account.data.clone(),
    )
}

fn state(
    key: Pubkey,
    owner: Pubkey,
    lamports: u64,
    executable: bool,
    data: Vec<u8>,
) -> ProviderExpectedAccountStateV3 {
    ProviderExpectedAccountStateV3 {
        key,
        owner,
        lamports,
        executable,
        data,
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;
    use dclutch_market_core_codec::StateBumpsV1;
    use dclutch_product_runtime_v2::{
        ContentId as ProductContentId, ResultDomainInputV2, compile_result_domain_v2,
        result_domain_record_bytes,
    };
    use dclutch_source_contract::{
        SOURCE_FAILURE_POLICY_RELEASE_ID_V2, SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
    };
    use solana_program::instruction::AccountMeta;

    fn key(tag: u8) -> Pubkey {
        Pubkey::new_from_array([tag; 32])
    }

    fn observation(slot: u64) -> crate::Observation {
        crate::Observation {
            slot,
            unix_timestamp: 1_800_000_000,
            finality: Finality::Finalized,
        }
    }

    fn account(
        slot: u64,
        key: Pubkey,
        owner: Pubkey,
        lamports: u64,
        data: Vec<u8>,
    ) -> ObservedAccount {
        ObservedAccount {
            observation: observation(slot),
            key,
            owner,
            lamports,
            executable: false,
            data,
        }
    }

    fn frame(
        count: usize,
        program_index: usize,
        program: Pubkey,
        signers: &[usize],
        writable: &[usize],
    ) -> Vec<AccountMeta> {
        (0..count)
            .map(|index| {
                let address = if index == program_index {
                    program
                } else {
                    key(u8::try_from(index + 1).expect("small frame"))
                };
                AccountMeta {
                    pubkey: address,
                    is_signer: signers.contains(&index),
                    is_writable: writable.contains(&index),
                }
            })
            .collect()
    }

    struct SubmitCase {
        instruction: Instruction,
        return_data: Vec<u8>,
        submitter_before: ObservedAccount,
        update_before: ObservedAccount,
        lifecycle_before: ObservedAccount,
        treasury_before: ObservedAccount,
        submitter_after: ObservedAccount,
        update_after: ObservedAccount,
        lifecycle_after: ObservedAccount,
        treasury_after: ObservedAccount,
        rent: Rent,
    }

    impl SubmitCase {
        fn input(&self) -> ProviderSubmitFinalizedInputV3<'_> {
            ProviderSubmitFinalizedInputV3 {
                instruction: &self.instruction,
                return_data_program: self.instruction.program_id,
                return_data: &self.return_data,
                finalized_slot: 20,
                transaction_fee_lamports: 5_000,
                lifecycle_top_up_lamports: 0,
                expected_provider_fee_lamports: 7_500,
                rent: &self.rent,
                writable: ProviderSubmitWritableAccountsV3 {
                    submitter_before: &self.submitter_before,
                    update_before: &self.update_before,
                    lifecycle_before: &self.lifecycle_before,
                    treasury_before: &self.treasury_before,
                    submitter_after: &self.submitter_after,
                    update_after: &self.update_after,
                    lifecycle_after: &self.lifecycle_after,
                    treasury_after: &self.treasury_after,
                },
            }
        }
    }

    fn submit_case() -> SubmitCase {
        let resolution = key(200);
        let receiver = key(201);
        let registry = key(202);
        let update = key(203);
        let submitter = key(204);
        let refund = key(205);
        let market = key(206);
        let source = key(207);
        let encoded = key(208);
        let provider_release = key(209).to_bytes();
        let release_set = key(210).to_bytes();
        let source_material = key(211).to_bytes();
        let (lifecycle, bump) = Pubkey::find_program_address(
            &[PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3, update.as_ref()],
            &resolution,
        );
        let authority = Pubkey::find_program_address(
            &[
                PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3,
                market.as_ref(),
                source.as_ref(),
                update.as_ref(),
            ],
            &resolution,
        )
        .0;
        let treasury = Pubkey::find_program_address(&[b"treasury", &[0]], &receiver).0;
        let mut accounts = frame(
            PROVIDER_SUBMIT_ACCOUNT_COUNT_V3,
            14,
            resolution,
            &[0, 1],
            &[0, 1, 2, 34],
        );
        for (index, address) in [
            (0, submitter),
            (1, update),
            (2, lifecycle),
            (3, authority),
            (4, refund),
            (5, market),
            (8, registry),
            (16, source),
            (27, receiver),
            (32, encoded),
            (34, treasury),
        ] {
            accounts[index].pubkey = address;
        }
        let body = vec![0xa5; 64];
        let request = ProviderSubmitRequestV3 {
            generation: 7,
            reclaim_after_unix_seconds: 1_900_000_000,
            market: market.to_bytes(),
            source_state: source.to_bytes(),
            lifecycle: lifecycle.to_bytes(),
            source_material,
            provider_release,
            update_account: update.to_bytes(),
            provider_submitter: submitter.to_bytes(),
            refund_recipient: refund.to_bytes(),
            release_set,
            registry_program: registry.to_bytes(),
            encoded_vaa: encoded.to_bytes(),
            post_body_digest: hash(&body).to_bytes(),
        };
        let mut data = request.to_bytes().expect("submit request").to_vec();
        data.extend_from_slice(&body);
        let rent = Rent::default();
        let lifecycle_lamports = rent.minimum_balance(PROVIDER_UPDATE_LIFECYCLE_BYTES_V3);
        let update_data = vec![0x33; 96];
        let update_rent = rent.minimum_balance(update_data.len());
        let provider_fee = 7_500;
        let receipt = ProviderSubmitReceiptV3 {
            request_digest: hash(&data[..PROVIDER_SUBMIT_REQUEST_BYTES_V3]).to_bytes(),
            lifecycle: lifecycle.to_bytes(),
            update_account: update.to_bytes(),
            update_digest: hash(&update_data).to_bytes(),
            provider_submitter: submitter.to_bytes(),
            update_authority: authority.to_bytes(),
            refund_recipient: refund.to_bytes(),
            provider_release,
            post_body_digest: request.post_body_digest,
            market: market.to_bytes(),
            generation: 7,
            posted_slot: 19,
            publish_time: 1_800_000_000,
            update_rent_lamports: update_rent,
            provider_fee_lamports: provider_fee,
        };
        let lifecycle_state = ProviderUpdateLifecycleV3::submitted(
            request,
            bump,
            authority.to_bytes(),
            registry.to_bytes(),
            receipt.update_digest,
            receipt.publish_time,
            receipt.posted_slot,
            update_rent,
            provider_fee,
        )
        .expect("lifecycle");
        let submitter_lamports = 10_000_000_000;
        let treasury_lamports = 4_000;
        SubmitCase {
            instruction: Instruction {
                program_id: resolution,
                accounts,
                data,
            },
            return_data: receipt.to_bytes().expect("receipt").to_vec(),
            submitter_before: account(
                10,
                submitter,
                system_program::ID,
                submitter_lamports,
                vec![],
            ),
            update_before: account(10, update, system_program::ID, 0, vec![]),
            lifecycle_before: account(
                10,
                lifecycle,
                system_program::ID,
                lifecycle_lamports,
                vec![],
            ),
            treasury_before: account(10, treasury, system_program::ID, treasury_lamports, vec![]),
            submitter_after: account(
                20,
                submitter,
                system_program::ID,
                submitter_lamports - 5_000 - update_rent - provider_fee,
                vec![],
            ),
            update_after: account(20, update, receiver, update_rent, update_data),
            lifecycle_after: account(
                20,
                lifecycle,
                resolution,
                lifecycle_lamports,
                lifecycle_state
                    .to_bytes()
                    .expect("lifecycle bytes")
                    .to_vec(),
            ),
            treasury_after: account(
                20,
                treasury,
                system_program::ID,
                treasury_lamports + provider_fee,
                vec![],
            ),
            rent,
        }
    }

    #[test]
    fn submit_projection_is_exact_and_refuses_receipt_poststate_and_slot_tamper() {
        let exact = submit_case();
        let projection =
            project_finalized_provider_submit_v3(exact.input()).expect("exact provider submit");
        assert_eq!(
            projection.expected_writable_poststates[1].data,
            exact.update_after.data
        );

        let mut wrong_receipt = submit_case();
        let mut receipt =
            ProviderSubmitReceiptV3::decode(&wrong_receipt.return_data).expect("typed receipt");
        receipt.posted_slot = 21;
        wrong_receipt.return_data = receipt.to_bytes().expect("receipt").to_vec();
        assert_eq!(
            project_finalized_provider_submit_v3(wrong_receipt.input()),
            Err(ProviderFinalizedProjectionErrorV3::ReturnData)
        );

        let mut wrong_post = submit_case();
        wrong_post.lifecycle_after.lamports += 1;
        assert_eq!(
            project_finalized_provider_submit_v3(wrong_post.input()),
            Err(ProviderFinalizedProjectionErrorV3::Poststate)
        );

        let mut wrong_slot = submit_case();
        wrong_slot.treasury_after.observation.slot = 19;
        assert_eq!(
            project_finalized_provider_submit_v3(wrong_slot.input()),
            Err(ProviderFinalizedProjectionErrorV3::Poststate)
        );

        let wrong_top_up = submit_case();
        let mut input = wrong_top_up.input();
        input.lifecycle_top_up_lamports = 1;
        assert_eq!(
            project_finalized_provider_submit_v3(input),
            Err(ProviderFinalizedProjectionErrorV3::Prestate)
        );

        let wrong_fee = submit_case();
        let mut input = wrong_fee.input();
        input.expected_provider_fee_lamports += 1;
        assert_eq!(
            project_finalized_provider_submit_v3(input),
            Err(ProviderFinalizedProjectionErrorV3::ReturnData)
        );
    }

    struct ReclaimCase {
        instruction: Instruction,
        return_data: Vec<u8>,
        certificate: ObservedAccount,
        lifecycle_before: ObservedAccount,
        update_before: ObservedAccount,
        authority_before: ObservedAccount,
        refund_before: ObservedAccount,
        lifecycle_after: ObservedAccount,
        update_after: ObservedAccount,
        authority_after: ObservedAccount,
        refund_after: ObservedAccount,
        rent: Rent,
    }

    impl ReclaimCase {
        fn input(&self) -> ProviderReclaimFinalizedInputV3<'_> {
            ProviderReclaimFinalizedInputV3 {
                instruction: &self.instruction,
                return_data_program: self.instruction.program_id,
                return_data: &self.return_data,
                finalized_slot: 30,
                execution_unix_timestamp: 1_800_000_100,
                rent: &self.rent,
                certificate: &self.certificate,
                writable: ProviderReclaimWritableAccountsV3 {
                    lifecycle_before: &self.lifecycle_before,
                    update_before: &self.update_before,
                    authority_before: &self.authority_before,
                    refund_before: &self.refund_before,
                    lifecycle_after: &self.lifecycle_after,
                    update_after: &self.update_after,
                    authority_after: &self.authority_after,
                    refund_after: &self.refund_after,
                },
            }
        }
    }

    fn reclaim_case() -> ReclaimCase {
        let resolution = key(180);
        let receiver = key(181);
        let registry = key(182);
        let update = key(183);
        let submitter = key(184);
        let refund = key(185);
        let resolver = key(186);
        let market = key(187);
        let source = key(188);
        let certificate_key = key(189);
        let provider_release = key(190).to_bytes();
        let release_set = key(191).to_bytes();
        let source_material = key(192).to_bytes();
        let (lifecycle_key, bump) = Pubkey::find_program_address(
            &[PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3, update.as_ref()],
            &resolution,
        );
        let authority = Pubkey::find_program_address(
            &[
                PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3,
                market.as_ref(),
                source.as_ref(),
                update.as_ref(),
            ],
            &resolution,
        )
        .0;
        let update_data = vec![0x42; 128];
        let rent = Rent::default();
        let update_rent = rent.minimum_balance(update_data.len());
        let submit_request = ProviderSubmitRequestV3 {
            generation: 7,
            reclaim_after_unix_seconds: 1_800_000_050,
            market: market.to_bytes(),
            source_state: source.to_bytes(),
            lifecycle: lifecycle_key.to_bytes(),
            source_material,
            provider_release,
            update_account: update.to_bytes(),
            provider_submitter: submitter.to_bytes(),
            refund_recipient: refund.to_bytes(),
            release_set,
            registry_program: registry.to_bytes(),
            encoded_vaa: key(193).to_bytes(),
            post_body_digest: key(194).to_bytes(),
        };
        let mut lifecycle = ProviderUpdateLifecycleV3::submitted(
            submit_request,
            bump,
            authority.to_bytes(),
            registry.to_bytes(),
            hash(&update_data).to_bytes(),
            1_800_000_000,
            20,
            update_rent,
            7_500,
        )
        .expect("submitted lifecycle");
        let provider_evidence = key(195).to_bytes();
        lifecycle
            .consume(3, provider_evidence, certificate_key.to_bytes())
            .expect("consumed lifecycle");
        let request = ProviderReclaimRequestV3 {
            generation: 7,
            terminal_sequence: 3,
            market: market.to_bytes(),
            source_state: source.to_bytes(),
            lifecycle: lifecycle_key.to_bytes(),
            certificate: certificate_key.to_bytes(),
            update_account: update.to_bytes(),
            resolver: resolver.to_bytes(),
            refund_recipient: refund.to_bytes(),
            release_set,
        };
        let request_bytes = request.to_bytes().expect("reclaim request");
        let receipt = ProviderReclaimReceiptV3 {
            request_digest: hash(&request_bytes).to_bytes(),
            lifecycle: lifecycle_key.to_bytes(),
            update_account: update.to_bytes(),
            certificate: certificate_key.to_bytes(),
            resolver: resolver.to_bytes(),
            refund_recipient: refund.to_bytes(),
            update_digest: lifecycle.update_digest,
            provider_evidence,
            generation: 7,
            terminal_sequence: 3,
            refunded_lamports: update_rent,
        };
        let certificate = ResolutionCertificateV2 {
            kind: ResolutionCertificateKindV2::ResolutionSuccess,
            market: market.to_bytes(),
            route: provider_release,
            source_material,
            product_record_digest: key(196).to_bytes(),
            provider_evidence,
            funding_allocation: [0; 32],
            receipt_account: certificate_key.to_bytes(),
            generation: 7,
            attempt_index: 0,
            schedule_index: 0,
            selector: 0,
            work_paid: 0,
            funding_remaining: 0,
            result_numerator: 1,
            result_denominator: 1,
            observed_at: 1_800_000_000,
        };
        let mut accounts = frame(
            PROVIDER_RECLAIM_ACCOUNT_COUNT_V3,
            9,
            resolution,
            &[0],
            &[1, 2, 3, 4],
        );
        for (index, address) in [
            (0, resolver),
            (1, lifecycle_key),
            (2, update),
            (3, authority),
            (4, refund),
            (5, certificate_key),
            (7, registry),
            (13, receiver),
        ] {
            accounts[index].pubkey = address;
        }
        let lifecycle_lamports = rent.minimum_balance(PROVIDER_UPDATE_LIFECYCLE_BYTES_V3);
        let refund_lamports = 10_000;
        ReclaimCase {
            instruction: Instruction {
                program_id: resolution,
                accounts,
                data: request_bytes.to_vec(),
            },
            return_data: receipt.to_bytes().expect("reclaim receipt").to_vec(),
            certificate: account(
                20,
                certificate_key,
                resolution,
                rent.minimum_balance(RESOLUTION_CERTIFICATE_BYTES_V2),
                certificate.to_bytes().expect("certificate").to_vec(),
            ),
            lifecycle_before: account(
                20,
                lifecycle_key,
                resolution,
                lifecycle_lamports,
                lifecycle.to_bytes().expect("lifecycle").to_vec(),
            ),
            update_before: account(20, update, receiver, update_rent, update_data),
            authority_before: account(20, authority, system_program::ID, 0, vec![]),
            refund_before: account(20, refund, system_program::ID, refund_lamports, vec![]),
            lifecycle_after: account(30, lifecycle_key, system_program::ID, 0, vec![]),
            update_after: account(30, update, system_program::ID, 0, vec![]),
            authority_after: account(30, authority, system_program::ID, 0, vec![]),
            refund_after: account(
                30,
                refund,
                system_program::ID,
                refund_lamports + update_rent + lifecycle_lamports,
                vec![],
            ),
            rent,
        }
    }

    #[test]
    fn reclaim_projection_is_exact_and_refuses_alias_time_receipt_and_one_lamport() {
        let exact = reclaim_case();
        let projection =
            project_finalized_provider_reclaim_v3(exact.input()).expect("exact provider reclaim");
        assert_eq!(
            projection.expected_writable_poststates[0].owner,
            system_program::ID
        );

        let mut aliased = reclaim_case();
        aliased.instruction.accounts[3].pubkey = aliased.instruction.accounts[2].pubkey;
        assert_eq!(
            project_finalized_provider_reclaim_v3(aliased.input()),
            Err(ProviderFinalizedProjectionErrorV3::Instruction)
        );

        let early = reclaim_case();
        let mut input = early.input();
        input.execution_unix_timestamp = 1_800_000_049;
        assert_eq!(
            project_finalized_provider_reclaim_v3(input),
            Err(ProviderFinalizedProjectionErrorV3::Prestate)
        );

        let mut changed_receipt = reclaim_case();
        let mut receipt =
            ProviderReclaimReceiptV3::decode(&changed_receipt.return_data).expect("typed receipt");
        receipt.provider_evidence = key(197).to_bytes();
        changed_receipt.return_data = receipt.to_bytes().expect("receipt").to_vec();
        assert_eq!(
            project_finalized_provider_reclaim_v3(changed_receipt.input()),
            Err(ProviderFinalizedProjectionErrorV3::ReturnData)
        );

        let mut one_lamport = reclaim_case();
        one_lamport.refund_after.lamports += 1;
        assert_eq!(
            project_finalized_provider_reclaim_v3(one_lamport.input()),
            Err(ProviderFinalizedProjectionErrorV3::Poststate)
        );
    }

    struct ExecuteCase {
        instruction: Instruction,
        return_data: Vec<u8>,
        source_material: ObservedAccount,
        result_domain: ObservedAccount,
        update: ObservedAccount,
        source_before: ObservedAccount,
        certificate_before: ObservedAccount,
        market_before: ObservedAccount,
        lifecycle_before: ObservedAccount,
        source_after: ObservedAccount,
        certificate_after: ObservedAccount,
        market_after: ObservedAccount,
        lifecycle_after: ObservedAccount,
        rent: Rent,
    }

    impl ExecuteCase {
        fn input(&self) -> ProviderExecuteFinalizedInputV3<'_> {
            ProviderExecuteFinalizedInputV3 {
                instruction: &self.instruction,
                return_data_program: self.instruction.accounts[15].pubkey,
                return_data: &self.return_data,
                finalized_slot: 30,
                execution_unix_timestamp: 1_800_000_010,
                rent: &self.rent,
                certificate_top_up_lamports: 0,
                source_material: &self.source_material,
                result_domain: &self.result_domain,
                update: &self.update,
                writable: ProviderExecuteWritableAccountsV3 {
                    source_before: &self.source_before,
                    certificate_before: &self.certificate_before,
                    market_before: &self.market_before,
                    lifecycle_before: &self.lifecycle_before,
                    source_after: &self.source_after,
                    certificate_after: &self.certificate_after,
                    market_after: &self.market_after,
                    lifecycle_after: &self.lifecycle_after,
                },
            }
        }
    }

    fn source_id(tag: u8) -> SourceContentId {
        SourceContentId::new(key(tag).to_bytes()).expect("source identity")
    }

    fn product_id(tag: u8) -> ProductContentId {
        ProductContentId::new(key(tag).to_bytes()).expect("product identity")
    }

    fn execute_case() -> ExecuteCase {
        let core = key(130);
        let resolution = key(131);
        let registry = key(132);
        let receiver = key(133);
        let resolver = key(134);
        let submitter = key(135);
        let update = key(136);
        let market_key = key(137);
        let generation = 7_u64;
        let product_record = source_id(138);
        let product_identity = product_id(139);
        let coordinate = product_id(140);
        let result_unit = product_id(141);
        let domain_input = ResultDomainInputV2 {
            product_id: product_identity,
            coordinate_domain_id: coordinate,
            result_unit_id: result_unit,
            liability_basis_id: product_id(142),
            representation_release_id: product_id(143),
            mapping_release_id: product_id(144),
            cut_denominator: 1,
            cuts: &[0],
        };
        let mut domain_bytes = vec![0; result_domain_record_bytes(1).expect("domain width")];
        compile_result_domain_v2(domain_input, &mut domain_bytes).expect("result domain");
        let domain = ResultDomainV2::decode(&domain_bytes).expect("result domain");
        let domain_digest = hash(&domain_bytes).to_bytes();
        let source_spec = source_id(145);
        let material = SourceMaterialV3::explicitly_unbounded(
            product_record,
            source_spec,
            source_id(146),
            source_id(147),
            None,
            SourceContentId::new(SOURCE_FAILURE_POLICY_RELEASE_ID_V2).expect("failure policy"),
        );
        let material_bytes = material.to_bytes().to_vec();
        let material_id = hash(&material_bytes).to_bytes();
        let (source_key, source_bump) = Pubkey::find_program_address(
            &[
                SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
                market_key.as_ref(),
                &generation.to_le_bytes(),
            ],
            &resolution,
        );
        let source = SourceResolutionStateV2::fresh(
            market_key.to_bytes(),
            generation,
            SourceContentId::new(material_id).expect("material ID"),
            key(148).to_bytes(),
            source_bump,
            0,
            0,
        )
        .expect("fresh Source")
        .state();
        let release_set = key(149).to_bytes();
        let provider_release = key(150).to_bytes();
        let registry_id =
            dclutch_market_core_codec::Identity::new(registry.to_bytes()).expect("registry");
        let market_id =
            dclutch_market_core_codec::Identity::new(market_key.to_bytes()).expect("market");
        let market = CoreState {
            phase: Phase::Open,
            readiness: Readiness::Consumed,
            terminal_winner: 0,
            identity: dclutch_market_core_codec::MarketIdentity {
                market_id,
                realm_id: dclutch_market_core_codec::Identity::new(key(151).to_bytes())
                    .expect("realm"),
                product_record: dclutch_market_core_codec::Identity::new(product_record.to_bytes())
                    .expect("product record"),
                product_id: dclutch_market_core_codec::Identity::new(product_identity.to_bytes())
                    .expect("product"),
                resolution_policy: dclutch_market_core_codec::Identity::new(material_id)
                    .expect("material"),
                capability_manifest: dclutch_market_core_codec::Identity::new(key(152).to_bytes())
                    .expect("manifest"),
                selected_release_set: dclutch_market_core_codec::Identity::new(release_set)
                    .expect("release set"),
                registry_program: registry_id,
                generation,
            },
            outstanding_capabilities: 0,
            principal_cap_sets: 1,
            rent_beneficiary: dclutch_market_core_codec::Identity::new(key(153).to_bytes())
                .expect("beneficiary"),
            terminal_receipt: None,
            bumps: StateBumpsV1::UNRECORDED,
        };
        let core_request = Request::administrative(Action::ExecuteProvider, generation, market_id);
        let core_bytes = core_request.encode().expect("Core request");
        let terminal_sequence = 3_u64;
        let (certificate_key, _) = Pubkey::find_program_address(
            &[
                RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
                source_key.as_ref(),
                &[1],
                &terminal_sequence.to_le_bytes(),
            ],
            &resolution,
        );
        let (lifecycle_key, lifecycle_bump) = Pubkey::find_program_address(
            &[PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3, update.as_ref()],
            &resolution,
        );
        let update_authority = Pubkey::find_program_address(
            &[
                PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3,
                market_key.as_ref(),
                source_key.as_ref(),
                update.as_ref(),
            ],
            &resolution,
        )
        .0;
        let body = vec![0x62; 64];
        let update_data = vec![0x63; 128];
        let update_digest = hash(&update_data).to_bytes();
        let provider_request = ProviderExecutionRequestV3 {
            caller: ProviderCallerV3::Core,
            generation,
            terminal_sequence,
            market: market_key.to_bytes(),
            source_state: source_key.to_bytes(),
            certificate_account: certificate_key.to_bytes(),
            source_material: material_id,
            source_spec: source_spec.to_bytes(),
            product_record: product_record.to_bytes(),
            result_domain: domain_digest,
            provider_release,
            update_account: update.to_bytes(),
            expected_update_digest: update_digest,
            provider_submitter: submitter.to_bytes(),
            resolver: resolver.to_bytes(),
            caller_program: core.to_bytes(),
            release_set,
            capability_program_set: [0; 32],
            selected_capability_program: [0; 32],
            parent_request_digest: hash(&core_bytes).to_bytes(),
            post_params_body_digest: hash(&body).to_bytes(),
        };
        let provider_bytes = provider_request.to_bytes().expect("provider request");
        let authority_seeds = CallerAuthoritySeedsV1::from_bytes(
            release_set,
            market_key.to_bytes(),
            ExecutionRoleV1::Core,
            source_key.to_bytes(),
            provider_request.parent_request_digest,
        )
        .expect("caller seeds");
        let caller_authority = Pubkey::find_program_address(&authority_seeds.as_slices(), &core).0;
        let mut accounts = frame(
            PROVIDER_EXECUTE_ACCOUNT_COUNT_V3,
            11,
            core,
            &[1],
            &[2, 3, 37],
        );
        for (index, address) in [
            (0, caller_authority),
            (1, resolver),
            (2, source_key),
            (3, certificate_key),
            (4, market_key),
            (7, registry),
            (15, resolution),
            (17, key(154)),
            (33, key(155)),
            (37, lifecycle_key),
            (38, update),
            (39, receiver),
        ] {
            accounts[index].pubkey = address;
        }
        let mut instruction_data = core_bytes.to_vec();
        instruction_data.extend_from_slice(&provider_bytes);
        instruction_data.extend_from_slice(&body);
        let evidence = hashv(&[
            PROVIDER_EVIDENCE_DOMAIN_V3,
            &[0],
            &provider_request.source_spec,
            &provider_release,
            &provider_request.update_account,
            &update_digest,
            &provider_request.post_params_body_digest,
        ])
        .to_bytes();
        let numerator = 1_i128;
        let selector = domain.select_ordinary(numerator, 1).expect("selector");
        let outcome_count = domain.outcome_count().expect("outcomes");
        let receipt = ProviderExecutionReceiptV3 {
            caller: ProviderCallerV3::Core,
            generation,
            terminal_sequence,
            request_digest: hash(&provider_bytes).to_bytes(),
            provider_evidence: evidence,
            update_digest,
            post_params_body_digest: provider_request.post_params_body_digest,
            market: market_key.to_bytes(),
            source_state: source_key.to_bytes(),
            certificate_account: certificate_key.to_bytes(),
            source_material: material_id,
            product_record: product_record.to_bytes(),
            result_domain: domain_digest,
            provider_release,
            update_account: update.to_bytes(),
            provider_submitter: submitter.to_bytes(),
            resolver: resolver.to_bytes(),
            caller_program: core.to_bytes(),
            release_set,
            capability_program_set: [0; 32],
            selected_capability_program: [0; 32],
            selector,
            outcome_count,
            result_numerator: numerator,
            result_denominator: 1,
            publish_time: 1_800_000_000,
            posted_slot: 20,
            consumed_slot: 30,
        };
        let submit_request = ProviderSubmitRequestV3 {
            generation,
            reclaim_after_unix_seconds: 1_800_000_100,
            market: market_key.to_bytes(),
            source_state: source_key.to_bytes(),
            lifecycle: lifecycle_key.to_bytes(),
            source_material: material_id,
            provider_release,
            update_account: update.to_bytes(),
            provider_submitter: submitter.to_bytes(),
            refund_recipient: key(156).to_bytes(),
            release_set,
            registry_program: registry.to_bytes(),
            encoded_vaa: key(157).to_bytes(),
            post_body_digest: provider_request.post_params_body_digest,
        };
        let rent = Rent::default();
        let mut lifecycle = ProviderUpdateLifecycleV3::submitted(
            submit_request,
            lifecycle_bump,
            update_authority.to_bytes(),
            registry.to_bytes(),
            update_digest,
            receipt.publish_time,
            receipt.posted_slot,
            rent.minimum_balance(update_data.len()),
            7_500,
        )
        .expect("submitted lifecycle");
        let lifecycle_before_bytes = lifecycle.to_bytes().expect("lifecycle").to_vec();
        lifecycle
            .consume(terminal_sequence, evidence, certificate_key.to_bytes())
            .expect("consume lifecycle");
        let mut source_after = source;
        source_after
            .resolve_primary_from_authenticated_domain(
                SourceContentId::new(material_id).expect("material"),
                material,
                product_record,
                domain,
                SourceContentId::new(evidence).expect("evidence"),
                numerator,
                1,
                generation,
                1_800_000_010,
                terminal_sequence,
            )
            .expect("resolve Source");
        let certificate = ResolutionCertificateV2 {
            kind: ResolutionCertificateKindV2::ResolutionSuccess,
            market: market_key.to_bytes(),
            route: provider_release,
            source_material: material_id,
            product_record_digest: product_record.to_bytes(),
            provider_evidence: evidence,
            funding_allocation: [0; 32],
            receipt_account: certificate_key.to_bytes(),
            generation,
            attempt_index: 0,
            schedule_index: 0,
            selector,
            work_paid: 0,
            funding_remaining: 0,
            result_numerator: numerator,
            result_denominator: 1,
            observed_at: 1_800_000_000,
        };
        let source_lamports = rent.minimum_balance(source.to_bytes().len());
        let certificate_lamports = rent.minimum_balance(RESOLUTION_CERTIFICATE_BYTES_V2);
        let market_lamports = rent.minimum_balance(market.encode().expect("market").len());
        let lifecycle_lamports = rent.minimum_balance(PROVIDER_UPDATE_LIFECYCLE_BYTES_V3);
        let market_before_bytes = market.encode().expect("market").to_vec();
        ExecuteCase {
            instruction: Instruction {
                program_id: core,
                accounts,
                data: instruction_data,
            },
            return_data: receipt.to_bytes().expect("receipt").to_vec(),
            source_material: account(20, key(154), registry, 1, material_bytes),
            result_domain: account(20, key(155), registry, 1, domain_bytes),
            update: account(
                20,
                update,
                receiver,
                rent.minimum_balance(update_data.len()),
                update_data,
            ),
            source_before: account(
                20,
                source_key,
                resolution,
                source_lamports,
                source.to_bytes().to_vec(),
            ),
            certificate_before: account(
                20,
                certificate_key,
                system_program::ID,
                certificate_lamports,
                vec![],
            ),
            market_before: account(
                20,
                market_key,
                core,
                market_lamports,
                market_before_bytes.clone(),
            ),
            lifecycle_before: account(
                20,
                lifecycle_key,
                resolution,
                lifecycle_lamports,
                lifecycle_before_bytes,
            ),
            source_after: account(
                30,
                source_key,
                resolution,
                source_lamports,
                source_after.to_bytes().to_vec(),
            ),
            certificate_after: account(
                30,
                certificate_key,
                resolution,
                certificate_lamports,
                certificate.to_bytes().expect("certificate").to_vec(),
            ),
            market_after: account(30, market_key, core, market_lamports, market_before_bytes),
            lifecycle_after: account(
                30,
                lifecycle_key,
                resolution,
                lifecycle_lamports,
                lifecycle.to_bytes().expect("lifecycle").to_vec(),
            ),
            rent,
        }
    }

    #[test]
    fn execute_projection_rebuilds_resolution_writes_and_requires_an_unchanged_open_market() {
        let exact = execute_case();
        let projection =
            project_finalized_provider_execute_v3(exact.input()).expect("exact provider execute");
        assert_eq!(
            projection.expected_writable_poststates[0].data,
            exact.source_after.data
        );
        assert_eq!(
            projection.expected_writable_poststates[2].data,
            exact.market_before.data
        );

        let mut wrong_source = execute_case();
        wrong_source.source_after.data[0] ^= 1;
        assert_eq!(
            project_finalized_provider_execute_v3(wrong_source.input()),
            Err(ProviderFinalizedProjectionErrorV3::Poststate)
        );

        let mut wrong_market = execute_case();
        wrong_market.market_after.data[0] ^= 1;
        wrong_market.market_after.observation = observation(30);
        assert_eq!(
            project_finalized_provider_execute_v3(wrong_market.input()),
            Err(ProviderFinalizedProjectionErrorV3::Poststate)
        );

        let mut wrong_receipt = execute_case();
        let mut receipt =
            ProviderExecutionReceiptV3::decode(&wrong_receipt.return_data).expect("typed receipt");
        receipt.consumed_slot = 31;
        wrong_receipt.return_data = receipt.to_bytes().expect("receipt").to_vec();
        assert_eq!(
            project_finalized_provider_execute_v3(wrong_receipt.input()),
            Err(ProviderFinalizedProjectionErrorV3::ReturnData)
        );

        let mut wrong_update = execute_case();
        wrong_update.update.data[0] ^= 1;
        assert_eq!(
            project_finalized_provider_execute_v3(wrong_update.input()),
            Err(ProviderFinalizedProjectionErrorV3::Prestate)
        );

        let wrong_top_up = execute_case();
        let mut input = wrong_top_up.input();
        input.certificate_top_up_lamports = 1;
        assert_eq!(
            project_finalized_provider_execute_v3(input),
            Err(ProviderFinalizedProjectionErrorV3::Prestate)
        );
    }
}
