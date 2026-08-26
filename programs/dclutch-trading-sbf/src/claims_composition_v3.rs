//! Family-neutral Claims CPI execution for EffectProgram V3 routes.
//!
//! [`ClaimsCompositionV3`] preflights the complete enabled Claims subsequence.
//! The common Trading outer may then call [`execute_claims_route_v3`] in global
//! EffectProgram order, interleaved with other fixed roles. Every invocation
//! uses the release-pinned Trading authority derived from the exact canonical
//! child request and immediately validates the current Claims producer receipt.

extern crate alloc;

use alloc::{boxed::Box, vec::Vec};

use dclutch_claims_svm::{
    affine_batch_v2::{AFFINE_BATCH_PLAN_MAGIC_V2, AffineBatchPlanV2, AffineBatchReceiptV2},
    composition_v3::ClaimsCompositionV3,
    founding_v5::{
        CLAIMS_FOUNDING_POST_RESOURCE_DIGEST_DOMAIN_V5, CLAIMS_FOUNDING_REQUEST_MAGIC_V5,
        ClaimsFoundingReceiptV5, ClaimsFoundingRequestV5,
    },
    protocol_position_v2::{
        PROTOCOL_POSITION_REQUEST_MAGIC_V2, ProtocolPositionActionV2, ProtocolPositionAdmissionV2,
        ProtocolPositionCloseReceiptV2, ProtocolPositionRequestV2,
    },
    signed_delta_v3::{SIGNED_DELTA_PLAN_MAGIC_V3, SignedDeltaPlanV3, SignedDeltaReceiptV3},
    sparse_native_transfer_v1::{
        SPARSE_NATIVE_TRANSFER_MAGIC_V1, SparseNativeTransferReceiptV1, SparseNativeTransferV1,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{ProgramV3, ResolvedInvocationV3, RouteKindV3},
};
use dclutch_rational_representation_v2_lifecycle_contract::{
    LIFECYCLE_REQUEST_MAGIC_V2, LifecycleReceiptV2, LifecycleRequestV2,
    hot_v3::verify_rational_lifecycle_hot_receipt_v3,
};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::{TradingSbfError, child_receipt_v3::append_receipt_dependency_v3};

/// Exact receipt returned by one canonical Claims route.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClaimsRouteReceiptV3 {
    /// Vacant canonical Position and admission record were admitted.
    Admit(ProtocolPositionAdmissionV2),
    /// Sole affine Claims mutation committed.
    Affine(AffineBatchReceiptV2),
    /// Canonical runtime-width unique signed-delta batch committed.
    SignedDelta(SignedDeltaReceiptV3),
    /// Canonical O(1) native-claim transfer committed.
    SparseNativeTransfer(SparseNativeTransferReceiptV1),
    /// Permit-authorized aggregate, founder Position, and admission were created.
    Founding(Box<ClaimsFoundingReceiptV5>),
    /// One exact Rational receipt/coordinate resource lifecycle committed.
    RationalLifecycle(LifecycleReceiptV2),
    /// Zero canonical Position and admission record were reclaimed.
    Close(ProtocolPositionCloseReceiptV2),
}

/// Invoke and verify one preflighted Claims route in global EffectProgram order.
#[allow(clippy::too_many_arguments)]
pub fn execute_claims_route_v3<'info>(
    program_id: &Pubkey,
    effect: ProgramV3<'_>,
    composition: ClaimsCompositionV3<'_>,
    route_index: u16,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    effect_accounts: &[AccountInfo<'info>],
    request_bank: &[u8],
    family_request: &[u8],
    prior_receipt: Option<&[u8]>,
    claims_program: &AccountInfo<'info>,
) -> Result<ClaimsRouteReceiptV3, ProgramError> {
    if effect
        .account_count(tail_count)
        .map_err(|_| TradingSbfError::Content)?
        != effect_accounts.len()
        || !claims_program.executable
        || claims_program.is_writable
        || claims_program.is_signer
    {
        return Err(TradingSbfError::Content.into());
    }
    let invocation = effect
        .resolved_invocation(route_index, 0, tail_count, scalars, identities)
        .map_err(|_| TradingSbfError::Content)?;
    if invocation.role != FixedRole::Claims || !composition_owns_route(composition, route_index) {
        return Err(TradingSbfError::Content.into());
    }
    let request = invocation_request(invocation, request_bank, family_request)?;
    let mut child_accounts = invocation_accounts(invocation, effect_accounts)?;
    if child_accounts.is_empty()
        || child_accounts
            .iter()
            .filter(|account| account.key == claims_program.key)
            .count()
            != 1
    {
        return Err(TradingSbfError::Content.into());
    }
    let (authority_seeds, receipt_kind) = route_authority(request, invocation.kind)?;
    let (expected_authority, bump) =
        Pubkey::find_program_address(&authority_seeds.as_slices(), program_id);
    if child_accounts
        .first()
        .is_none_or(|account| account.key != &expected_authority)
    {
        return Err(TradingSbfError::Release.into());
    }

    let mut metas = Vec::with_capacity(child_accounts.len());
    for (index, account) in child_accounts.iter().enumerate() {
        metas.push(child_account_meta_v3(index, account));
    }
    let mut child_data = request.to_vec();
    append_receipt_dependency_v3(invocation, &mut child_data, prior_receipt)?;
    let instruction = Instruction {
        program_id: *claims_program.key,
        accounts: metas,
        data: child_data,
    };
    child_accounts.push(claims_program.clone());
    let bump_seed = [bump];
    let [domain, release, market, role, context, digest] = authority_seeds.as_slices();
    invoke_signed(
        &instruction,
        &child_accounts,
        &[&[domain, release, market, role, context, digest, &bump_seed]],
    )
    .map_err(|_| TradingSbfError::Transition)?;
    let (producer, receipt) = get_return_data().ok_or(TradingSbfError::Transition)?;
    if producer != *claims_program.key {
        return Err(TradingSbfError::Transition.into());
    }
    let post_resources = match receipt_kind {
        ReceiptKindV3::SignedDelta => {
            PostResourceEvidenceV3::Single(signed_delta_post_resource_digest(
                &child_accounts,
                SignedDeltaPlanV3::decode(request)
                    .map_err(|_| TradingSbfError::Content)?
                    .position_count(),
            )?)
        }
        ReceiptKindV3::SparseNativeTransfer => {
            PostResourceEvidenceV3::Single(sparse_native_post_resource_digest(&child_accounts)?)
        }
        ReceiptKindV3::Founding => {
            PostResourceEvidenceV3::Founding(founding_post_resource_digests(&child_accounts)?)
        }
        _ => PostResourceEvidenceV3::None,
    };
    verify_route_receipt(
        receipt_kind,
        request,
        &receipt,
        claims_program.key.to_bytes(),
        program_id.to_bytes(),
        post_resources,
    )
}

fn child_account_meta_v3(index: usize, account: &AccountInfo<'_>) -> AccountMeta {
    let signer = index == 0 || account.is_signer;
    if account.is_writable {
        AccountMeta::new(*account.key, signer)
    } else {
        AccountMeta::new_readonly(*account.key, signer)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReceiptKindV3 {
    Admit,
    Affine,
    SignedDelta,
    SparseNativeTransfer,
    Founding,
    RationalLifecycle,
    Close,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FoundingPostResourceDigestsV5 {
    aggregate: [u8; 32],
    position: [u8; 32],
    admission: [u8; 32],
    combined: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PostResourceEvidenceV3 {
    None,
    Single([u8; 32]),
    Founding(FoundingPostResourceDigestsV5),
}

fn composition_owns_route(composition: ClaimsCompositionV3<'_>, route: u16) -> bool {
    composition.admit_route() == Some(route)
        || composition.mutation_route() == route
        || composition.close_route() == Some(route)
}

fn invocation_request<'a>(
    invocation: ResolvedInvocationV3,
    request_bank: &'a [u8],
    family_request: &'a [u8],
) -> Result<&'a [u8], ProgramError> {
    let end = invocation
        .request_offset
        .checked_add(invocation.request_len)
        .ok_or(TradingSbfError::Content)?;
    let fixed = request_bank
        .get(invocation.request_offset..end)
        .ok_or(TradingSbfError::Content)?;
    match invocation.borrowed_witness {
        None => Ok(fixed),
        Some(witness) if fixed.is_empty() => {
            let request = witness
                .slice(family_request)
                .map_err(|_| TradingSbfError::Content)?;
            if request.get(..8) == Some(SIGNED_DELTA_PLAN_MAGIC_V3.as_slice()) {
                let plan =
                    SignedDeltaPlanV3::decode(request).map_err(|_| TradingSbfError::Content)?;
                let parent = family_request
                    .get(..witness.source_offset())
                    .ok_or(TradingSbfError::Content)?;
                if hash(parent).to_bytes() != plan.request_id() {
                    return Err(TradingSbfError::Content.into());
                }
            }
            Ok(request)
        }
        Some(_) => Err(TradingSbfError::Content.into()),
    }
}

fn invocation_accounts<'info>(
    invocation: ResolvedInvocationV3,
    accounts: &[AccountInfo<'info>],
) -> Result<Vec<AccountInfo<'info>>, ProgramError> {
    let mut output = Vec::new();
    let fixed_start = usize::from(invocation.fixed_account_start);
    let fixed_end = fixed_start
        .checked_add(usize::from(invocation.fixed_account_count))
        .ok_or(TradingSbfError::Content)?;
    output.extend_from_slice(
        accounts
            .get(fixed_start..fixed_end)
            .ok_or(TradingSbfError::Content)?,
    );
    if invocation.kind == RouteKindV3::AffineOnce {
        let count = usize::from(invocation.item_account_count);
        let stride = usize::from(invocation.item_account_stride);
        let mut item = 0_u32;
        while item < invocation.repeated_item_count {
            let index = usize::try_from(item).map_err(|_| TradingSbfError::Content)?;
            let start = invocation
                .item_account_start
                .checked_add(index.checked_mul(stride).ok_or(TradingSbfError::Content)?)
                .ok_or(TradingSbfError::Content)?;
            let end = start.checked_add(count).ok_or(TradingSbfError::Content)?;
            output.extend_from_slice(accounts.get(start..end).ok_or(TradingSbfError::Content)?);
            item = item.checked_add(1).ok_or(TradingSbfError::Content)?;
        }
    } else if invocation.item_account_count != 0 || invocation.repeated_item_count != 0 {
        return Err(TradingSbfError::Content.into());
    }
    Ok(output)
}

fn route_authority(
    request: &[u8],
    kind: RouteKindV3,
) -> Result<(CallerAuthoritySeedsV1, ReceiptKindV3), ProgramError> {
    let packet_digest = hash(request).to_bytes();
    if request.get(..8) == Some(PROTOCOL_POSITION_REQUEST_MAGIC_V2.as_slice()) {
        if kind != RouteKindV3::Once {
            return Err(TradingSbfError::Content.into());
        }
        let position =
            ProtocolPositionRequestV2::decode(request).map_err(|_| TradingSbfError::Content)?;
        let seeds = CallerAuthoritySeedsV1::new(
            ContentId::new(position.release_set).map_err(|_| TradingSbfError::Content)?,
            position.market,
            ExecutionRoleV1::Trading,
            position.position_owner,
            packet_digest,
        )
        .map_err(|_| TradingSbfError::Content)?;
        let receipt = match position.action {
            ProtocolPositionActionV2::Admit => ReceiptKindV3::Admit,
            ProtocolPositionActionV2::Close => ReceiptKindV3::Close,
        };
        Ok((seeds, receipt))
    } else if request.get(..8) == Some(AFFINE_BATCH_PLAN_MAGIC_V2.as_slice()) {
        if kind != RouteKindV3::AffineOnce {
            return Err(TradingSbfError::Content.into());
        }
        let plan = AffineBatchPlanV2::decode(request).map_err(|_| TradingSbfError::Content)?;
        let seeds = CallerAuthoritySeedsV1::new(
            ContentId::new(plan.release_set()).map_err(|_| TradingSbfError::Content)?,
            plan.market(),
            ExecutionRoleV1::Trading,
            plan.request_id(),
            packet_digest,
        )
        .map_err(|_| TradingSbfError::Content)?;
        Ok((seeds, ReceiptKindV3::Affine))
    } else if request.get(..8) == Some(SIGNED_DELTA_PLAN_MAGIC_V3.as_slice()) {
        if kind != RouteKindV3::Once {
            return Err(TradingSbfError::Content.into());
        }
        let plan = SignedDeltaPlanV3::decode(request).map_err(|_| TradingSbfError::Content)?;
        let seeds = CallerAuthoritySeedsV1::new(
            ContentId::new(plan.release_set()).map_err(|_| TradingSbfError::Content)?,
            plan.market(),
            ExecutionRoleV1::Trading,
            plan.request_id(),
            packet_digest,
        )
        .map_err(|_| TradingSbfError::Content)?;
        Ok((seeds, ReceiptKindV3::SignedDelta))
    } else if request.get(..8) == Some(SPARSE_NATIVE_TRANSFER_MAGIC_V1.as_slice()) {
        if kind != RouteKindV3::Once {
            return Err(TradingSbfError::Content.into());
        }
        let request =
            SparseNativeTransferV1::decode(request).map_err(|_| TradingSbfError::Content)?;
        let input = request.input();
        if input.caller_role != dclutch_claims_svm::CallerRole::Trading {
            return Err(TradingSbfError::Content.into());
        }
        let seeds = CallerAuthoritySeedsV1::new(
            ContentId::new(input.release_set).map_err(|_| TradingSbfError::Content)?,
            input.market,
            ExecutionRoleV1::Trading,
            input.request_id,
            packet_digest,
        )
        .map_err(|_| TradingSbfError::Content)?;
        Ok((seeds, ReceiptKindV3::SparseNativeTransfer))
    } else if request.get(..8) == Some(CLAIMS_FOUNDING_REQUEST_MAGIC_V5.as_slice()) {
        if kind != RouteKindV3::Once {
            return Err(TradingSbfError::Content.into());
        }
        let request =
            ClaimsFoundingRequestV5::decode(request).map_err(|_| TradingSbfError::Content)?;
        let seeds = CallerAuthoritySeedsV1::new(
            ContentId::new(request.release_set()).map_err(|_| TradingSbfError::Content)?,
            request.market(),
            ExecutionRoleV1::Trading,
            request.founding_intent_digest(),
            packet_digest,
        )
        .map_err(|_| TradingSbfError::Content)?;
        Ok((seeds, ReceiptKindV3::Founding))
    } else if request.get(..8) == Some(LIFECYCLE_REQUEST_MAGIC_V2.as_slice()) {
        if kind != RouteKindV3::Once {
            return Err(TradingSbfError::Content.into());
        }
        let request = LifecycleRequestV2::decode(request).map_err(|_| TradingSbfError::Content)?;
        let header = request.header();
        let seeds = CallerAuthoritySeedsV1::new(
            ContentId::new(header.release_set).map_err(|_| TradingSbfError::Content)?,
            header.market,
            ExecutionRoleV1::Trading,
            header.parent_context,
            packet_digest,
        )
        .map_err(|_| TradingSbfError::Content)?;
        Ok((seeds, ReceiptKindV3::RationalLifecycle))
    } else {
        Err(TradingSbfError::Content.into())
    }
}

fn verify_route_receipt(
    kind: ReceiptKindV3,
    request: &[u8],
    receipt: &[u8],
    claims_program: [u8; 32],
    trading_program: [u8; 32],
    expected_post_resources: PostResourceEvidenceV3,
) -> Result<ClaimsRouteReceiptV3, ProgramError> {
    let request_digest = hash(request).to_bytes();
    match kind {
        ReceiptKindV3::Admit => verify_admit_receipt(
            request,
            receipt,
            request_digest,
            claims_program,
            trading_program,
        ),
        ReceiptKindV3::Affine => {
            verify_affine_receipt(request, receipt, request_digest, claims_program)
        }
        ReceiptKindV3::SignedDelta => verify_signed_delta_receipt(
            request,
            receipt,
            request_digest,
            claims_program,
            match expected_post_resources {
                PostResourceEvidenceV3::Single(value) => Some(value),
                _ => None,
            },
        ),
        ReceiptKindV3::SparseNativeTransfer => verify_sparse_native_receipt(
            request,
            receipt,
            request_digest,
            claims_program,
            match expected_post_resources {
                PostResourceEvidenceV3::Single(value) => Some(value),
                _ => None,
            },
        ),
        ReceiptKindV3::Founding => verify_founding_receipt(
            request,
            receipt,
            request_digest,
            claims_program,
            trading_program,
            match expected_post_resources {
                PostResourceEvidenceV3::Founding(value) => Some(value),
                _ => None,
            },
        ),
        ReceiptKindV3::RationalLifecycle => {
            verify_rational_lifecycle_receipt(request, receipt, request_digest)
        }
        ReceiptKindV3::Close => {
            verify_close_receipt(request, receipt, request_digest, claims_program)
        }
    }
}

#[inline(never)]
fn verify_rational_lifecycle_receipt(
    request: &[u8],
    receipt: &[u8],
    request_digest: [u8; 32],
) -> Result<ClaimsRouteReceiptV3, ProgramError> {
    let request = LifecycleRequestV2::decode(request).map_err(|_| TradingSbfError::Content)?;
    let receipt = verify_rational_lifecycle_hot_receipt_v3(request, request_digest, receipt)
        .map_err(|_| TradingSbfError::Transition)?;
    Ok(ClaimsRouteReceiptV3::RationalLifecycle(receipt))
}

#[inline(never)]
fn verify_founding_receipt(
    request: &[u8],
    receipt: &[u8],
    request_digest: [u8; 32],
    claims_program: [u8; 32],
    trading_program: [u8; 32],
    expected: Option<FoundingPostResourceDigestsV5>,
) -> Result<ClaimsRouteReceiptV3, ProgramError> {
    let request = ClaimsFoundingRequestV5::decode(request).map_err(|_| TradingSbfError::Content)?;
    let receipt = Box::new(
        ClaimsFoundingReceiptV5::decode(receipt).map_err(|_| TradingSbfError::Transition)?,
    );
    receipt
        .verify_for(&request, request_digest)
        .map_err(|_| TradingSbfError::Transition)?;
    let expected = expected.ok_or(TradingSbfError::Transition)?;
    if request.claims_program() != claims_program
        || request.trading_program() != trading_program
        || receipt.aggregate_digest() != expected.aggregate
        || receipt.position_digest() != expected.position
        || receipt.admission_digest() != expected.admission
        || receipt.post_resource_digest() != expected.combined
    {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(ClaimsRouteReceiptV3::Founding(receipt))
}

#[inline(never)]
fn verify_admit_receipt(
    request: &[u8],
    receipt: &[u8],
    request_digest: [u8; 32],
    claims_program: [u8; 32],
    trading_program: [u8; 32],
) -> Result<ClaimsRouteReceiptV3, ProgramError> {
    let request =
        ProtocolPositionRequestV2::decode(request).map_err(|_| TradingSbfError::Content)?;
    let receipt = ProtocolPositionAdmissionV2::decode_receipt(receipt)
        .map_err(|_| TradingSbfError::Transition)?;
    receipt
        .validate_request(request, request_digest, claims_program, trading_program)
        .map_err(|_| TradingSbfError::Transition)?;
    Ok(ClaimsRouteReceiptV3::Admit(receipt))
}

#[inline(never)]
fn verify_affine_receipt(
    request: &[u8],
    receipt: &[u8],
    request_digest: [u8; 32],
    claims_program: [u8; 32],
) -> Result<ClaimsRouteReceiptV3, ProgramError> {
    let plan = AffineBatchPlanV2::decode(request).map_err(|_| TradingSbfError::Content)?;
    let receipt = AffineBatchReceiptV2::decode(receipt).map_err(|_| TradingSbfError::Transition)?;
    receipt
        .validate_plan(plan)
        .map_err(|_| TradingSbfError::Transition)?;
    if receipt.packet_digest() != request_digest || receipt.claims_program() != claims_program {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(ClaimsRouteReceiptV3::Affine(receipt))
}

#[inline(never)]
fn verify_signed_delta_receipt(
    request: &[u8],
    receipt: &[u8],
    request_digest: [u8; 32],
    claims_program: [u8; 32],
    expected_post_resource_digest: Option<[u8; 32]>,
) -> Result<ClaimsRouteReceiptV3, ProgramError> {
    let plan = SignedDeltaPlanV3::decode(request).map_err(|_| TradingSbfError::Content)?;
    let receipt = SignedDeltaReceiptV3::decode(receipt).map_err(|_| TradingSbfError::Transition)?;
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
    if receipt.packet_digest() != request_digest
        || receipt.table_digest() != table_digest
        || receipt.claims_program() != claims_program
        || Some(receipt.post_resource_digest()) != expected_post_resource_digest
    {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(ClaimsRouteReceiptV3::SignedDelta(receipt))
}

#[inline(never)]
fn verify_sparse_native_receipt(
    request: &[u8],
    receipt: &[u8],
    request_digest: [u8; 32],
    claims_program: [u8; 32],
    expected_post_resource_digest: Option<[u8; 32]>,
) -> Result<ClaimsRouteReceiptV3, ProgramError> {
    let request = SparseNativeTransferV1::decode(request).map_err(|_| TradingSbfError::Content)?;
    let receipt =
        SparseNativeTransferReceiptV1::decode(receipt).map_err(|_| TradingSbfError::Transition)?;
    receipt
        .validate_request(request)
        .map_err(|_| TradingSbfError::Transition)?;
    if receipt.packet_digest() != request_digest
        || receipt.claims_program() != claims_program
        || Some(receipt.post_resource_digest()) != expected_post_resource_digest
    {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(ClaimsRouteReceiptV3::SparseNativeTransfer(receipt))
}

#[inline(never)]
fn verify_close_receipt(
    request: &[u8],
    receipt: &[u8],
    request_digest: [u8; 32],
    claims_program: [u8; 32],
) -> Result<ClaimsRouteReceiptV3, ProgramError> {
    let request =
        ProtocolPositionRequestV2::decode(request).map_err(|_| TradingSbfError::Content)?;
    let receipt =
        ProtocolPositionCloseReceiptV2::decode(receipt).map_err(|_| TradingSbfError::Transition)?;
    receipt
        .validate_request(request, request_digest, claims_program)
        .map_err(|_| TradingSbfError::Transition)?;
    Ok(ClaimsRouteReceiptV3::Close(receipt))
}

fn signed_delta_post_resource_digest(
    child_accounts: &[AccountInfo<'_>],
    position_count: u32,
) -> Result<[u8; 32], ProgramError> {
    let positions = usize::try_from(position_count).map_err(|_| TradingSbfError::Content)?;
    let expected = 21_usize
        .checked_add(positions)
        .ok_or(TradingSbfError::Content)?;
    if child_accounts.len() != expected {
        return Err(TradingSbfError::Content.into());
    }
    let mut preimage = Vec::new();
    preimage.extend_from_slice(b"dclutch/claims/signed-delta-post-resources/v3");
    let market = child_accounts
        .get(1)
        .ok_or(TradingSbfError::Content)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    preimage.extend_from_slice(&market);
    let end = 20_usize
        .checked_add(positions)
        .ok_or(TradingSbfError::Content)?;
    for account in child_accounts
        .get(20..end)
        .ok_or(TradingSbfError::Content)?
    {
        let data = account
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Transition)?;
        preimage.extend_from_slice(&data);
    }
    Ok(hash(&preimage).to_bytes())
}

fn sparse_native_post_resource_digest(
    child_accounts: &[AccountInfo<'_>],
) -> Result<[u8; 32], ProgramError> {
    if child_accounts.len() != 23 {
        return Err(TradingSbfError::Content.into());
    }
    let mut preimage = Vec::new();
    preimage.extend_from_slice(b"dclutch/claims/sparse-native-post/v1");
    for index in [1_usize, 20, 21] {
        let data = child_accounts
            .get(index)
            .ok_or(TradingSbfError::Content)?
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Transition)?;
        preimage.extend_from_slice(&data);
    }
    Ok(hash(&preimage).to_bytes())
}

fn founding_post_resource_digests(
    child_accounts: &[AccountInfo<'_>],
) -> Result<FoundingPostResourceDigestsV5, ProgramError> {
    // The exact FoundingV5 frame is 32 accounts; the CPI program account is
    // appended once by this adapter for invoke_signed.
    if child_accounts.len() != 33 {
        return Err(TradingSbfError::Content.into());
    }
    let aggregate_data = child_accounts
        .get(2)
        .ok_or(TradingSbfError::Content)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let position_data = child_accounts
        .get(3)
        .ok_or(TradingSbfError::Content)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let admission_data = child_accounts
        .get(4)
        .ok_or(TradingSbfError::Content)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    Ok(FoundingPostResourceDigestsV5 {
        aggregate: hash(&aggregate_data).to_bytes(),
        position: hash(&position_data).to_bytes(),
        admission: hash(&admission_data).to_bytes(),
        combined: hashv(&[
            CLAIMS_FOUNDING_POST_RESOURCE_DIGEST_DOMAIN_V5,
            &aggregate_data,
            &position_data,
            &admission_data,
        ])
        .to_bytes(),
    })
}

#[cfg(test)]
mod tests {
    use alloc::boxed::Box;

    use dclutch_claims_svm::{
        CallerRole,
        affine_batch_v2::{
            AffineBatchPlanInputV2, AffineBatchPositionV2, AffineBatchRowInputV2, AffineBatchRowV2,
            DeltaDirectionV2, SignedMagnitudeV2, plan_bytes,
        },
        founding_v5::{ClaimsFoundingRequestInputV5, ClaimsFoundingRequestV5},
        protocol_position_v2::{
            ProtocolPositionAdmissionEvidenceV2, ProtocolPositionCloseEvidenceV2,
            ProtocolPositionOwnerKindV2, ProtocolPositionPresenceV2,
        },
        signed_delta_v3::{
            DeltaDirectionV3, PositionDeltaInputV3, PositionDeltaV3, SignedDeltaPlanInputV3,
            SignedDeltaPositionV3, SignedDeltaV3, plan_bytes as signed_plan_bytes,
        },
        sparse_native_transfer_v1::{
            SPARSE_NATIVE_TRANSFER_RECEIPT_BYTES_V1, SparseNativeTransferInputV1,
            SparseNativeTransferV1,
        },
    };

    use super::*;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn account_info(signer: bool, writable: bool) -> AccountInfo<'static> {
        let key = Box::leak(Box::new(Pubkey::new_unique()));
        let owner = Box::leak(Box::new(Pubkey::new_unique()));
        let lamports = Box::leak(Box::new(0_u64));
        let data = Box::leak(Vec::<u8>::new().into_boxed_slice());
        AccountInfo::new(key, signer, writable, lamports, data, owner, false)
    }

    #[test]
    fn authority_signing_is_added_and_existing_actor_signer_is_preserved() {
        let authority = account_info(false, false);
        let ordinary = account_info(false, true);
        let actor = account_info(true, false);

        assert!(child_account_meta_v3(0, &authority).is_signer);
        assert!(!child_account_meta_v3(1, &ordinary).is_signer);
        assert!(child_account_meta_v3(3, &actor).is_signer);
        assert!(!child_account_meta_v3(3, &ordinary).is_signer);
    }

    fn position(action: ProtocolPositionActionV2) -> ProtocolPositionRequestV2 {
        ProtocolPositionRequestV2 {
            action,
            owner_kind: ProtocolPositionOwnerKindV2::User,
            presence: match action {
                ProtocolPositionActionV2::Admit => ProtocolPositionPresenceV2::Vacant,
                ProtocolPositionActionV2::Close => ProtocolPositionPresenceV2::Existing,
            },
            release_set: id(1),
            market: id(2),
            position_owner: id(3),
            parent_request_digest: id(4),
            rent_credit: id(5),
            rent_program: id(6),
            generation: 7,
            expected_market_revision: 8,
            expected_position_revision: if action == ProtocolPositionActionV2::Admit {
                0
            } else {
                9
            },
            observed_position_lamports: 12,
            observed_admission_lamports: 13,
            position_rent_principal: 10,
            admission_rent_principal: 11,
            capability_descriptor: [0; 32],
            capability_outcome: 0,
        }
        .new()
        .expect("position request")
    }

    fn delta(direction: DeltaDirectionV2, magnitude: u64) -> SignedMagnitudeV2 {
        SignedMagnitudeV2::new(direction, magnitude).expect("delta")
    }

    fn affine_bytes() -> Vec<u8> {
        let positions = [
            AffineBatchPositionV2::new(id(3), 8).expect("source"),
            AffineBatchPositionV2::new(id(9), 0).expect("destination"),
        ];
        let rows = [AffineBatchRowV2::new(
            AffineBatchRowInputV2 {
                source_present: true,
                destination_present: true,
                outcome: 0,
                source_position_index: 0,
                destination_position_index: 1,
                aggregate_delta: delta(DeltaDirectionV2::Neutral, 0),
                source_delta: delta(DeltaDirectionV2::Debit, 5),
                destination_delta: delta(DeltaDirectionV2::Credit, 5),
            },
            2,
            2,
        )
        .expect("row")];
        let mut bytes = alloc::vec![0; plan_bytes(2, 1).expect("width")];
        AffineBatchPlanV2::encode_into(
            AffineBatchPlanInputV2 {
                caller_role: CallerRole::Trading,
                release_set: id(1),
                market: id(2),
                request_id: id(4),
                product_record_digest: id(10),
                semantic_basis_id: id(11),
                linked_basis_record_digest: id(12),
                expected_market_revision: 8,
                outcome_count: 2,
            },
            &positions,
            &rows,
            &mut bytes,
        )
        .expect("affine");
        bytes
    }

    fn signed_bytes() -> Vec<u8> {
        let positions = [
            SignedDeltaPositionV3::new(id(3), 8).expect("source"),
            SignedDeltaPositionV3::new(id(9), 4).expect("destination"),
        ];
        let aggregates = [
            SignedDeltaV3::new(DeltaDirectionV3::Neutral, 0).expect("zero"),
            SignedDeltaV3::new(DeltaDirectionV3::Neutral, 0).expect("zero"),
        ];
        let rows = [
            PositionDeltaV3::new(
                PositionDeltaInputV3 {
                    position_index: 0,
                    outcome: 1,
                    delta: SignedDeltaV3::new(DeltaDirectionV3::Debit, 5).expect("debit"),
                },
                2,
                2,
            )
            .expect("source row"),
            PositionDeltaV3::new(
                PositionDeltaInputV3 {
                    position_index: 1,
                    outcome: 1,
                    delta: SignedDeltaV3::new(DeltaDirectionV3::Credit, 5).expect("credit"),
                },
                2,
                2,
            )
            .expect("destination row"),
        ];
        let mut bytes = alloc::vec![0; signed_plan_bytes(2, 2, 2).expect("width")];
        SignedDeltaPlanV3::encode_into(
            SignedDeltaPlanInputV3 {
                caller_role: CallerRole::Trading,
                release_set: id(1),
                market: id(2),
                request_id: id(4),
                product_record_digest: id(10),
                semantic_basis_id: id(11),
                linked_basis_record_digest: id(12),
                expected_market_revision: 8,
                claim_count: 2,
            },
            &positions,
            &aggregates,
            &rows,
            &mut bytes,
        )
        .expect("signed");
        bytes
    }

    fn sparse() -> SparseNativeTransferV1 {
        SparseNativeTransferV1::new(SparseNativeTransferInputV1 {
            caller_role: CallerRole::Trading,
            release_set: id(1),
            market: id(2),
            request_id: id(4),
            product_record_digest: id(10),
            semantic_basis_id: id(11),
            linked_basis_record_digest: id(12),
            source_owner: id(3),
            destination_owner: id(9),
            expected_market_revision: 8,
            expected_source_revision: 4,
            expected_destination_revision: 6,
            generation: 7,
            outcome: 1,
            claim_count: 2,
            quantity: 5,
        })
        .expect("sparse request")
    }

    fn founding(claims: [u8; 32], trading: [u8; 32]) -> ClaimsFoundingRequestV5 {
        ClaimsFoundingRequestV5::new(ClaimsFoundingRequestInputV5 {
            release_set: id(1),
            market: id(2),
            product_record_digest: id(30),
            product_instance_id: id(31),
            linked_basis_record_digest: id(32),
            semantic_basis_id: id(33),
            founder: id(34),
            founding_intent_digest: id(35),
            aggregate: id(36),
            position: id(37),
            admission: id(38),
            funding_source: id(39),
            hoard: id(40),
            custody_replay: id(41),
            rent_credit: id(42),
            rent_program: id(43),
            claims_program: claims,
            trading_program: trading,
            custody_request_digest: id(46),
            custody_receipt_digest: id(47),
            generation: 7,
            claim_count: 2,
            quantity: 2,
            basis_scale: 5,
            pre_source_amount: 10,
            post_source_amount: 0,
            pre_hoard_amount: 7,
            post_hoard_amount: 17,
            pre_custody_revision: 0,
            post_custody_revision: 1,
            aggregate_rent_principal: 100,
            position_rent_principal: 101,
            admission_rent_principal: 102,
            observed_aggregate_lamports: 100,
            observed_position_lamports: 101,
            observed_admission_lamports: 102,
            pre_aggregate_revision: 0,
            post_aggregate_revision: 1,
            pre_position_revision: 0,
            post_position_revision: 1,
        })
        .expect("founding request")
    }

    #[test]
    fn verifies_each_exact_claims_receipt_and_refuses_producer_substitution() {
        let claims = id(20);
        let trading = id(21);
        let admit = position(ProtocolPositionActionV2::Admit);
        let admit_bytes = admit.to_bytes().expect("admit bytes");
        let admission = ProtocolPositionAdmissionV2::new(
            admit,
            ProtocolPositionAdmissionEvidenceV2 {
                product_record_digest: id(30),
                semantic_basis_id: id(31),
                linked_basis_record_digest: id(32),
                request_digest: hash(&admit_bytes).to_bytes(),
                claims_program: claims,
                trading_program: trading,
                capability_descriptor: [0; 32],
                capability_outcome: 0,
                outcome_count: 258,
            },
        )
        .expect("admission");
        assert!(matches!(
            verify_route_receipt(
                ReceiptKindV3::Admit,
                &admit_bytes,
                &admission.to_receipt_bytes().expect("receipt"),
                claims,
                trading,
                PostResourceEvidenceV3::None,
            ),
            Ok(ClaimsRouteReceiptV3::Admit(_))
        ));
        assert!(
            verify_route_receipt(
                ReceiptKindV3::Admit,
                &admit_bytes,
                &admission.to_receipt_bytes().expect("receipt"),
                id(99),
                trading,
                PostResourceEvidenceV3::None,
            )
            .is_err()
        );

        let affine_bytes = affine_bytes();
        let affine = AffineBatchPlanV2::decode(&affine_bytes).expect("plan");
        let affine_receipt = AffineBatchReceiptV2::new(
            affine,
            hash(&affine_bytes).to_bytes(),
            id(40),
            claims,
            id(41),
            9,
        )
        .expect("affine receipt")
        .to_bytes();
        assert!(matches!(
            verify_route_receipt(
                ReceiptKindV3::Affine,
                &affine_bytes,
                &affine_receipt,
                claims,
                trading,
                PostResourceEvidenceV3::None,
            ),
            Ok(ClaimsRouteReceiptV3::Affine(_))
        ));

        let close = position(ProtocolPositionActionV2::Close);
        let close_bytes = close.to_bytes().expect("close bytes");
        let close_receipt = ProtocolPositionCloseReceiptV2::new(
            close,
            ProtocolPositionCloseEvidenceV2 {
                request_digest: hash(&close_bytes).to_bytes(),
                admission_digest: id(50),
                claims_program: claims,
                post_resource_digest: id(51),
                rent_credit_before: 100,
                rent_credit_after: 125,
            },
        )
        .expect("close receipt")
        .to_bytes()
        .expect("close bytes");
        assert!(matches!(
            verify_route_receipt(
                ReceiptKindV3::Close,
                &close_bytes,
                &close_receipt,
                claims,
                trading,
                PostResourceEvidenceV3::None,
            ),
            Ok(ClaimsRouteReceiptV3::Close(_))
        ));
    }

    #[test]
    fn derives_nonaliasing_authority_contexts_from_exact_child_packets() {
        let admit = position(ProtocolPositionActionV2::Admit)
            .to_bytes()
            .expect("admit");
        let affine = affine_bytes();
        let (admit_seeds, admit_kind) =
            route_authority(&admit, RouteKindV3::Once).expect("admit authority");
        let (affine_seeds, affine_kind) =
            route_authority(&affine, RouteKindV3::AffineOnce).expect("affine authority");
        assert_eq!(admit_kind, ReceiptKindV3::Admit);
        assert_eq!(affine_kind, ReceiptKindV3::Affine);
        let program = Pubkey::new_from_array(id(21));
        assert_ne!(
            Pubkey::find_program_address(&admit_seeds.as_slices(), &program).0,
            Pubkey::find_program_address(&affine_seeds.as_slices(), &program).0,
        );
        assert!(route_authority(&admit, RouteKindV3::AffineOnce).is_err());
        assert!(route_authority(&affine, RouteKindV3::Once).is_err());
    }

    #[test]
    fn verifies_founding_authority_and_exact_post_resources() {
        let claims = id(20);
        let trading = id(21);
        let request = founding(claims, trading);
        let request_bytes = request.to_bytes();
        let mut accounts: Vec<_> = (0..33).map(|_| account_info(false, false)).collect();
        accounts[2] = account_info(false, true);
        accounts[3] = account_info(false, true);
        accounts[4] = account_info(false, true);
        let post = founding_post_resource_digests(&accounts).expect("post resources");
        let receipt = ClaimsFoundingReceiptV5::new(
            request,
            hash(&request_bytes).to_bytes(),
            post.aggregate,
            post.position,
            post.admission,
            post.combined,
        )
        .expect("founding receipt")
        .to_bytes();

        assert!(matches!(
            verify_route_receipt(
                ReceiptKindV3::Founding,
                &request_bytes,
                &receipt,
                claims,
                trading,
                PostResourceEvidenceV3::Founding(post),
            ),
            Ok(ClaimsRouteReceiptV3::Founding(_))
        ));
        for (producer, selected_trading, evidence) in [
            (id(99), trading, PostResourceEvidenceV3::Founding(post)),
            (claims, id(99), PostResourceEvidenceV3::Founding(post)),
            (
                claims,
                trading,
                PostResourceEvidenceV3::Founding(FoundingPostResourceDigestsV5 {
                    combined: id(99),
                    ..post
                }),
            ),
        ] {
            assert!(
                verify_route_receipt(
                    ReceiptKindV3::Founding,
                    &request_bytes,
                    &receipt,
                    producer,
                    selected_trading,
                    evidence,
                )
                .is_err()
            );
        }

        let (seeds, kind) =
            route_authority(&request_bytes, RouteKindV3::Once).expect("founding authority");
        assert_eq!(kind, ReceiptKindV3::Founding);
        assert_eq!(seeds.context(), request.founding_intent_digest());
        assert!(route_authority(&request_bytes, RouteKindV3::AffineOnce).is_err());
        assert!(founding_post_resource_digests(&accounts[..32]).is_err());
    }

    #[test]
    fn verifies_signed_delta_table_producer_and_post_resources() {
        let request = signed_bytes();
        let plan = SignedDeltaPlanV3::decode(&request).expect("plan");
        let (positions, aggregates, deltas) = plan.table_bytes();
        let table_digest = hashv(&[
            b"dclutch/claims/signed-delta-table/v3",
            positions,
            aggregates,
            deltas,
        ])
        .to_bytes();
        let claims = id(20);
        let post_resources = id(21);
        let receipt = SignedDeltaReceiptV3::new(
            plan,
            hash(&request).to_bytes(),
            table_digest,
            claims,
            post_resources,
            9,
        )
        .expect("receipt")
        .to_bytes();
        assert!(matches!(
            verify_route_receipt(
                ReceiptKindV3::SignedDelta,
                &request,
                &receipt,
                claims,
                id(30),
                PostResourceEvidenceV3::Single(post_resources),
            ),
            Ok(ClaimsRouteReceiptV3::SignedDelta(_))
        ));
        assert!(
            verify_route_receipt(
                ReceiptKindV3::SignedDelta,
                &request,
                &receipt,
                claims,
                id(30),
                PostResourceEvidenceV3::Single(id(99)),
            )
            .is_err()
        );
    }

    #[test]
    fn verifies_sparse_authority_producer_packet_and_post_resources() {
        let request = sparse();
        let request_bytes = request.to_bytes();
        let claims = id(20);
        let post_resources = id(21);
        let receipt = SparseNativeTransferReceiptV1::new(
            request,
            hash(&request_bytes).to_bytes(),
            claims,
            post_resources,
            9,
            5,
            7,
        )
        .expect("sparse receipt")
        .to_bytes();
        assert_eq!(receipt.len(), SPARSE_NATIVE_TRANSFER_RECEIPT_BYTES_V1);
        assert!(matches!(
            verify_route_receipt(
                ReceiptKindV3::SparseNativeTransfer,
                &request_bytes,
                &receipt,
                claims,
                id(30),
                PostResourceEvidenceV3::Single(post_resources),
            ),
            Ok(ClaimsRouteReceiptV3::SparseNativeTransfer(_))
        ));
        assert!(
            verify_route_receipt(
                ReceiptKindV3::SparseNativeTransfer,
                &request_bytes,
                &receipt[..receipt.len() - 1],
                claims,
                id(30),
                PostResourceEvidenceV3::Single(post_resources),
            )
            .is_err()
        );
        for (producer, poststate) in [(id(99), post_resources), (claims, id(99))] {
            assert!(
                verify_route_receipt(
                    ReceiptKindV3::SparseNativeTransfer,
                    &request_bytes,
                    &receipt,
                    producer,
                    id(30),
                    PostResourceEvidenceV3::Single(poststate),
                )
                .is_err()
            );
        }

        let program = Pubkey::new_from_array(id(30));
        let (authority, kind) =
            route_authority(&request_bytes, RouteKindV3::Once).expect("sparse authority");
        assert_eq!(kind, ReceiptKindV3::SparseNativeTransfer);
        assert!(route_authority(&request_bytes, RouteKindV3::AffineOnce).is_err());
        let mut changed = request.input();
        changed.quantity = 6;
        let changed = SparseNativeTransferV1::new(changed)
            .expect("changed request")
            .to_bytes();
        let (changed_authority, _) =
            route_authority(&changed, RouteKindV3::Once).expect("changed authority");
        assert_ne!(
            Pubkey::find_program_address(&authority.as_slices(), &program).0,
            Pubkey::find_program_address(&changed_authority.as_slices(), &program).0,
        );
    }

    #[test]
    fn sparse_resource_digest_binds_exact_aggregate_source_destination_poststates() {
        let mut accounts: Vec<_> = (0..23).map(|_| account_info(false, false)).collect();
        accounts[1] = account_info(false, true);
        accounts[20] = account_info(false, true);
        accounts[21] = account_info(false, true);
        let expected = hashv(&[b"dclutch/claims/sparse-native-post/v1", &[], &[], &[]]).to_bytes();
        assert_eq!(sparse_native_post_resource_digest(&accounts), Ok(expected));
        assert!(sparse_native_post_resource_digest(&accounts[..22]).is_err());
    }
}
