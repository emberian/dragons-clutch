//! Family-neutral Custody CPI execution for EffectProgram V3 routes.

extern crate alloc;

use alloc::vec::Vec;

use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{CallerRoleV1, CustodyReceiptV1, CustodyRequestV1};
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{ProgramV3, ResolvedInvocationV3, RouteKindV3},
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

use crate::TradingSbfError;

const CUSTODY_EXECUTION_DIGEST_DOMAIN_V3: &[u8] = b"dclutch:hot-custody-receipt:v3";
const CUSTODY_REPLAY_FRAME_COORDINATE_V1: usize = 8;

/// Immutable parent facts every projected Custody request must reproduce.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CustodyCompositionParentV3 {
    /// Current immutable release set.
    pub release_set: [u8; 32],
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Current Market generation.
    pub generation: u64,
    /// SHA-256 of the complete exact family request.
    pub parent_request_digest: [u8; 32],
    /// Current Registry-selected Trading program.
    pub trading_program: [u8; 32],
}

/// Preflight one exact active Custody invocation without external mutation.
#[allow(clippy::too_many_arguments)]
pub fn preflight_custody_route_v3(
    program_id: &Pubkey,
    effect: ProgramV3<'_>,
    route_index: u16,
    invocation_index: u32,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    effect_accounts: &[AccountInfo<'_>],
    request_bank: &[u8],
    custody_program: &AccountInfo<'_>,
    parent: CustodyCompositionParentV3,
) -> Result<(), ProgramError> {
    let prepared = prepare(
        program_id,
        effect,
        route_index,
        invocation_index,
        tail_count,
        scalars,
        identities,
        effect_accounts,
        request_bank,
        custody_program,
        parent,
    )?;
    let _ = prepared;
    Ok(())
}

/// Execute one preflighted Custody invocation and verify its immediate receipt.
#[allow(clippy::too_many_arguments)]
pub fn execute_custody_route_v3<'info>(
    program_id: &Pubkey,
    effect: ProgramV3<'_>,
    route_index: u16,
    invocation_index: u32,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    effect_accounts: &[AccountInfo<'info>],
    request_bank: &[u8],
    custody_program: &AccountInfo<'info>,
    parent: CustodyCompositionParentV3,
) -> Result<[u8; 32], ProgramError> {
    let prepared = prepare(
        program_id,
        effect,
        route_index,
        invocation_index,
        tail_count,
        scalars,
        identities,
        effect_accounts,
        request_bank,
        custody_program,
        parent,
    )?;
    let mut child_accounts = invocation_accounts(prepared.invocation, effect_accounts)?;
    let mut metas = Vec::with_capacity(child_accounts.len());
    for (index, account) in child_accounts.iter().enumerate() {
        let signer = index == 0 || account.is_signer;
        metas.push(if account.is_writable {
            AccountMeta::new(*account.key, signer)
        } else {
            AccountMeta::new_readonly(*account.key, signer)
        });
    }
    let instruction = Instruction {
        program_id: *custody_program.key,
        accounts: metas,
        data: prepared.request_bytes.to_vec(),
    };
    child_accounts.push(custody_program.clone());
    let bump_seed = [prepared.bump];
    let [domain, release, market, role, context, digest] = prepared.authority_seeds.as_slices();
    invoke_signed(
        &instruction,
        &child_accounts,
        &[&[domain, release, market, role, context, digest, &bump_seed]],
    )
    .map_err(|_| TradingSbfError::Transition)?;
    let (producer, receipt_bytes) = get_return_data().ok_or(TradingSbfError::Transition)?;
    if producer != *custody_program.key {
        return Err(TradingSbfError::Transition.into());
    }
    let receipt =
        CustodyReceiptV1::decode(&receipt_bytes).map_err(|_| TradingSbfError::Transition)?;
    let replay = child_accounts
        .get(CUSTODY_REPLAY_FRAME_COORDINATE_V1)
        .ok_or(TradingSbfError::Transition)?;
    let replay_digest = {
        let bytes = replay
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Transition)?;
        hash(&bytes).to_bytes()
    };
    receipt
        .verify_for(prepared.request, prepared.request_digest, replay_digest)
        .map_err(|_| TradingSbfError::Transition)?;
    Ok(hashv(&[
        CUSTODY_EXECUTION_DIGEST_DOMAIN_V3,
        &route_index.to_le_bytes(),
        &invocation_index.to_le_bytes(),
        &prepared.request_digest,
        &receipt_bytes,
    ])
    .to_bytes())
}

struct PreparedCustodyInvocationV3<'a> {
    invocation: ResolvedInvocationV3,
    request: CustodyRequestV1,
    request_bytes: &'a [u8],
    request_digest: [u8; 32],
    authority_seeds: CallerAuthoritySeedsV1,
    bump: u8,
}

#[allow(clippy::too_many_arguments)]
fn prepare<'a>(
    program_id: &Pubkey,
    effect: ProgramV3<'_>,
    route_index: u16,
    invocation_index: u32,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    effect_accounts: &[AccountInfo<'_>],
    request_bank: &'a [u8],
    custody_program: &AccountInfo<'_>,
    parent: CustodyCompositionParentV3,
) -> Result<PreparedCustodyInvocationV3<'a>, ProgramError> {
    validate_parent(program_id, parent)?;
    if !custody_program.executable
        || custody_program.is_signer
        || custody_program.is_writable
        || effect
            .account_count(tail_count)
            .map_err(|_| TradingSbfError::Content)?
            != effect_accounts.len()
    {
        return Err(TradingSbfError::Content.into());
    }
    let invocation = effect
        .resolved_invocation(
            route_index,
            invocation_index,
            tail_count,
            scalars,
            identities,
        )
        .map_err(|_| TradingSbfError::Content)?;
    if invocation.role != FixedRole::Custody || invocation.borrowed_witness.is_some() {
        return Err(TradingSbfError::Content.into());
    }
    let request_bytes = invocation_request(invocation, request_bank)?;
    let request = CustodyRequestV1::decode(request_bytes).map_err(|_| TradingSbfError::Content)?;
    if request.caller_role != CallerRoleV1::Trading
        || request.release_set != parent.release_set
        || request.market != parent.market
        || request.semantic.generation != parent.generation
        || request.semantic.parent_request_digest != parent.parent_request_digest
        || request.caller_program != parent.trading_program
    {
        return Err(TradingSbfError::Content.into());
    }
    let request_digest = hash(request_bytes).to_bytes();
    let authority_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(request.release_set).map_err(|_| TradingSbfError::Content)?,
        request.market,
        ExecutionRoleV1::Trading,
        request.context,
        request_digest,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let (expected_authority, bump) =
        Pubkey::find_program_address(&authority_seeds.as_slices(), program_id);
    let child_accounts = invocation_accounts(invocation, effect_accounts)?;
    if child_accounts
        .first()
        .is_none_or(|account| account.key != &expected_authority)
        || child_accounts
            .iter()
            .filter(|account| account.key == custody_program.key)
            .count()
            != 1
        || child_accounts
            .get(CUSTODY_REPLAY_FRAME_COORDINATE_V1)
            .is_none()
    {
        return Err(TradingSbfError::Release.into());
    }
    Ok(PreparedCustodyInvocationV3 {
        invocation,
        request,
        request_bytes,
        request_digest,
        authority_seeds,
        bump,
    })
}

fn validate_parent(
    program_id: &Pubkey,
    parent: CustodyCompositionParentV3,
) -> Result<(), ProgramError> {
    if parent.release_set == [0; 32]
        || parent.market == [0; 32]
        || parent.parent_request_digest == [0; 32]
        || parent.trading_program != program_id.to_bytes()
    {
        Err(TradingSbfError::Content.into())
    } else {
        Ok(())
    }
}

fn invocation_request(
    invocation: ResolvedInvocationV3,
    request_bank: &[u8],
) -> Result<&[u8], ProgramError> {
    let end = invocation
        .request_offset
        .checked_add(invocation.request_len)
        .ok_or(TradingSbfError::Content)?;
    request_bank
        .get(invocation.request_offset..end)
        .ok_or_else(|| TradingSbfError::Content.into())
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
    let item_count = usize::from(invocation.item_account_count);
    match invocation.kind {
        RouteKindV3::Once => {
            if item_count != 0 || invocation.repeated_item_count != 0 {
                return Err(TradingSbfError::Content.into());
            }
        }
        RouteKindV3::Each => {
            if invocation.item.is_none() || invocation.repeated_item_count != 1 {
                return Err(TradingSbfError::Content.into());
            }
            let end = invocation
                .item_account_start
                .checked_add(item_count)
                .ok_or(TradingSbfError::Content)?;
            output.extend_from_slice(
                accounts
                    .get(invocation.item_account_start..end)
                    .ok_or(TradingSbfError::Content)?,
            );
        }
        RouteKindV3::AffineOnce => {
            let stride = usize::from(invocation.item_account_stride);
            let mut item = 0_u32;
            while item < invocation.repeated_item_count {
                let start = invocation
                    .item_account_start
                    .checked_add(
                        usize::try_from(item)
                            .map_err(|_| TradingSbfError::Content)?
                            .checked_mul(stride)
                            .ok_or(TradingSbfError::Content)?,
                    )
                    .ok_or(TradingSbfError::Content)?;
                let end = start
                    .checked_add(item_count)
                    .ok_or(TradingSbfError::Content)?;
                output.extend_from_slice(accounts.get(start..end).ok_or(TradingSbfError::Content)?);
                item = item.checked_add(1).ok_or(TradingSbfError::Content)?;
            }
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_binding_refuses_program_or_request_substitution() {
        let program = Pubkey::new_from_array([5; 32]);
        let canonical = CustodyCompositionParentV3 {
            release_set: [1; 32],
            market: [2; 32],
            generation: 3,
            parent_request_digest: [4; 32],
            trading_program: [5; 32],
        };
        assert_eq!(validate_parent(&program, canonical), Ok(()));

        for hostile in [
            CustodyCompositionParentV3 {
                trading_program: [6; 32],
                ..canonical
            },
            CustodyCompositionParentV3 {
                parent_request_digest: [0; 32],
                ..canonical
            },
            CustodyCompositionParentV3 {
                release_set: [0; 32],
                ..canonical
            },
            CustodyCompositionParentV3 {
                market: [0; 32],
                ..canonical
            },
        ] {
            assert_eq!(
                validate_parent(&program, hostile),
                Err(TradingSbfError::Content.into())
            );
        }
    }
}
