//! Family-neutral Claims CPI execution for EffectProgram V3 routes.
//!
//! [`ClaimsCompositionV3`] preflights the complete enabled Claims subsequence.
//! The common Trading outer may then call [`execute_claims_route_v3`] in global
//! EffectProgram order, interleaved with other fixed roles. Every invocation
//! uses the release-pinned Trading authority derived from the exact canonical
//! child request and immediately validates the current Claims producer receipt.

extern crate alloc;

use alloc::vec::Vec;

use dclutch_claims_svm::{
    affine_batch_v2::{AffineBatchPlanV2, AffineBatchReceiptV2, AFFINE_BATCH_PLAN_MAGIC_V2},
    composition_v3::ClaimsCompositionV3,
    protocol_position_v2::{
        ProtocolPositionActionV2, ProtocolPositionAdmissionV2, ProtocolPositionCloseReceiptV2,
        ProtocolPositionRequestV2, PROTOCOL_POSITION_REQUEST_MAGIC_V2,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{ProgramV3, ResolvedInvocationV3, RouteKindV3},
};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{
    account_info::AccountInfo,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::TradingSbfError;

/// Exact receipt returned by one canonical Claims route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimsRouteReceiptV3 {
    /// Vacant canonical Position and admission record were admitted.
    Admit(ProtocolPositionAdmissionV2),
    /// Sole affine Claims mutation committed.
    Affine(AffineBatchReceiptV2),
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
    let request = invocation_request(invocation, request_bank)?;
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
        let signer = index == 0;
        metas.push(if account.is_writable {
            AccountMeta::new(*account.key, signer)
        } else {
            AccountMeta::new_readonly(*account.key, signer)
        });
    }
    let instruction = Instruction {
        program_id: *claims_program.key,
        accounts: metas,
        data: request.to_vec(),
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
    verify_route_receipt(
        receipt_kind,
        request,
        &receipt,
        claims_program.key.to_bytes(),
        program_id.to_bytes(),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReceiptKindV3 {
    Admit,
    Affine,
    Close,
}

fn composition_owns_route(composition: ClaimsCompositionV3<'_>, route: u16) -> bool {
    composition.admit_route() == Some(route)
        || composition.affine_route() == route
        || composition.close_route() == Some(route)
}

fn invocation_request<'a>(
    invocation: ResolvedInvocationV3,
    request_bank: &'a [u8],
) -> Result<&'a [u8], ProgramError> {
    let end = invocation
        .request_offset
        .checked_add(invocation.request_len)
        .ok_or(TradingSbfError::Content)?;
    request_bank
        .get(invocation.request_offset..end)
        .ok_or_else(|| TradingSbfError::Content.into())
}

fn invocation_accounts<'accounts, 'info>(
    invocation: ResolvedInvocationV3,
    accounts: &'accounts [AccountInfo<'info>],
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
) -> Result<ClaimsRouteReceiptV3, ProgramError> {
    let request_digest = hash(request).to_bytes();
    match kind {
        ReceiptKindV3::Admit => {
            let request =
                ProtocolPositionRequestV2::decode(request).map_err(|_| TradingSbfError::Content)?;
            let receipt = ProtocolPositionAdmissionV2::decode_receipt(receipt)
                .map_err(|_| TradingSbfError::Transition)?;
            receipt
                .validate_request(request, request_digest, claims_program, trading_program)
                .map_err(|_| TradingSbfError::Transition)?;
            Ok(ClaimsRouteReceiptV3::Admit(receipt))
        }
        ReceiptKindV3::Affine => {
            let plan = AffineBatchPlanV2::decode(request).map_err(|_| TradingSbfError::Content)?;
            let receipt =
                AffineBatchReceiptV2::decode(receipt).map_err(|_| TradingSbfError::Transition)?;
            receipt
                .validate_plan(plan)
                .map_err(|_| TradingSbfError::Transition)?;
            if receipt.packet_digest() != request_digest
                || receipt.claims_program() != claims_program
            {
                return Err(TradingSbfError::Transition.into());
            }
            Ok(ClaimsRouteReceiptV3::Affine(receipt))
        }
        ReceiptKindV3::Close => {
            let request =
                ProtocolPositionRequestV2::decode(request).map_err(|_| TradingSbfError::Content)?;
            let receipt = ProtocolPositionCloseReceiptV2::decode(receipt)
                .map_err(|_| TradingSbfError::Transition)?;
            receipt
                .validate_request(request, request_digest, claims_program)
                .map_err(|_| TradingSbfError::Transition)?;
            Ok(ClaimsRouteReceiptV3::Close(receipt))
        }
    }
}

#[cfg(test)]
mod tests {
    use dclutch_claims_svm::{
        affine_batch_v2::{
            plan_bytes, AffineBatchPlanInputV2, AffineBatchPositionV2, AffineBatchRowInputV2,
            AffineBatchRowV2, DeltaDirectionV2, SignedMagnitudeV2,
        },
        protocol_position_v2::{
            ProtocolPositionAdmissionEvidenceV2, ProtocolPositionCloseEvidenceV2,
            ProtocolPositionOwnerKindV2, ProtocolPositionPresenceV2,
        },
        CallerRole,
    };

    use super::*;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
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
            ),
            Ok(ClaimsRouteReceiptV3::Admit(_))
        ));
        assert!(verify_route_receipt(
            ReceiptKindV3::Admit,
            &admit_bytes,
            &admission.to_receipt_bytes().expect("receipt"),
            id(99),
            trading,
        )
        .is_err());

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
}
