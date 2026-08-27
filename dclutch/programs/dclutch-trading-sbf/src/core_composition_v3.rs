//! Family-neutral Core CPI execution for EffectProgram V3 routes.
//!
//! The selected EffectProgram owns the exact request/account frame. This
//! adapter currently admits the canonical recurring-Series Core request ABI;
//! it appends only the EffectProgram-authenticated borrowed witness, signs the
//! release-pinned Trading authority, and accepts only the immediate current
//! Core producer's typed acknowledgment.

extern crate alloc;

use alloc::vec::Vec;

use dclutch_core_contract::ContentId;
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{ProgramV3, ResolvedInvocationV3, RouteKindV3},
};
use dclutch_market_core_codec::{
    Identity, SERIES_CORE_REQUEST_MAGIC_V1, SeriesCoreAckV1, SeriesCoreActionV1,
    SeriesCoreRequestV1,
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

use crate::{
    TradingSbfError,
    child_receipt_v3::{ReceiptDeliveryV3, deliver_receipt_dependency_v3},
    hot_v3::DowngradedEffectAccountsV3,
};

const CORE_EXECUTION_DIGEST_DOMAIN_V3: &[u8] = b"dclutch:hot-core-receipt:v3";

/// Immutable parent facts every projected Core request must reproduce.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoreCompositionParentV3 {
    /// Current immutable release set.
    pub release_set: [u8; 32],
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Current Market generation.
    pub generation: u64,
    /// Current Registry-selected Trading program.
    pub trading_program: [u8; 32],
}

/// Preflight one exact active Core invocation without external mutation.
#[allow(clippy::too_many_arguments)]
pub fn preflight_core_route_v3(
    program_id: &Pubkey,
    effect: ProgramV3<'_>,
    route_index: u16,
    invocation_index: u32,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    effect_accounts: DowngradedEffectAccountsV3<'_, '_, '_>,
    request_bank: &[u8],
    family_request: &[u8],
    core_program: &AccountInfo<'_>,
    parent: CoreCompositionParentV3,
) -> Result<(), ProgramError> {
    let _ = prepare(
        program_id,
        effect,
        route_index,
        invocation_index,
        tail_count,
        scalars,
        identities,
        effect_accounts,
        request_bank,
        family_request,
        core_program,
        parent,
    )?;
    Ok(())
}

/// Execute one preflighted Core invocation and verify its immediate receipt.
#[allow(clippy::too_many_arguments)]
pub fn execute_core_route_v3<'info>(
    program_id: &Pubkey,
    effect: ProgramV3<'_>,
    route_index: u16,
    invocation_index: u32,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    effect_accounts: DowngradedEffectAccountsV3<'_, '_, 'info>,
    request_bank: &[u8],
    family_request: &[u8],
    prior_receipt: Option<&[u8]>,
    core_program: &AccountInfo<'info>,
    parent: CoreCompositionParentV3,
) -> Result<[u8; 32], ProgramError> {
    let mut prepared = prepare(
        program_id,
        effect,
        route_index,
        invocation_index,
        tail_count,
        scalars,
        identities,
        effect_accounts,
        request_bank,
        family_request,
        core_program,
        parent,
    )?;
    // Core is the one hot-path child whose ABI genuinely READS the producer
    // receipt: `core-sbf::process_instruction`'s SERIES_CORE_REQUEST_MAGIC_V1
    // branch splits a trailing CLAIMS_FOUNDING_RECEIPT_MAGIC_V5 or
    // PROJECTED_CUSTODY_LOCK_RECEIPT_MAGIC_V1 off the tail and refuses the
    // request outright when neither is there. The suffix is that ABI, not a
    // Trading convenience, so it stays.
    deliver_receipt_dependency_v3(
        prepared.invocation,
        &mut prepared.child_data,
        prior_receipt,
        ReceiptDeliveryV3::ExactSuffix,
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
        program_id: *core_program.key,
        accounts: metas,
        data: prepared.child_data.clone(),
    };
    child_accounts.push(core_program.clone());
    let bump_seed = [prepared.bump];
    let [domain, release, market, role, context, digest] = prepared.authority_seeds.as_slices();
    invoke_signed(
        &instruction,
        &child_accounts,
        &[&[domain, release, market, role, context, digest, &bump_seed]],
    )
    .map_err(|_| TradingSbfError::Transition)?;
    let (producer, receipt_bytes) = get_return_data().ok_or(TradingSbfError::Transition)?;
    if producer != *core_program.key {
        return Err(TradingSbfError::Transition.into());
    }
    let receipt =
        SeriesCoreAckV1::decode(&receipt_bytes).map_err(|_| TradingSbfError::Transition)?;
    receipt
        .validate_for(
            prepared.request,
            Identity::new(core_program.key.to_bytes()).map_err(|_| TradingSbfError::Transition)?,
            Identity::new(prepared.request_digest).map_err(|_| TradingSbfError::Transition)?,
            receipt.post_resource_digest(),
        )
        .map_err(|_| TradingSbfError::Transition)?;
    Ok(hashv(&[
        CORE_EXECUTION_DIGEST_DOMAIN_V3,
        &route_index.to_le_bytes(),
        &invocation_index.to_le_bytes(),
        &hash(&prepared.child_data).to_bytes(),
        &receipt_bytes,
    ])
    .to_bytes())
}

struct PreparedCoreInvocationV3 {
    invocation: ResolvedInvocationV3,
    request: SeriesCoreRequestV1,
    child_data: Vec<u8>,
    request_digest: [u8; 32],
    authority_seeds: CallerAuthoritySeedsV1,
    bump: u8,
}

#[allow(clippy::too_many_arguments)]
fn prepare(
    program_id: &Pubkey,
    effect: ProgramV3<'_>,
    route_index: u16,
    invocation_index: u32,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    effect_accounts: DowngradedEffectAccountsV3<'_, '_, '_>,
    request_bank: &[u8],
    family_request: &[u8],
    core_program: &AccountInfo<'_>,
    parent: CoreCompositionParentV3,
) -> Result<PreparedCoreInvocationV3, ProgramError> {
    if parent.release_set == [0; 32]
        || parent.market == [0; 32]
        || parent.trading_program != program_id.to_bytes()
        || !core_program.executable
        || core_program.is_signer
        || core_program.is_writable
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
    if invocation.role != FixedRole::Core
        || invocation.kind != RouteKindV3::Once
        || invocation.item.is_some()
        || invocation.repeated_item_count != 0
    {
        return Err(TradingSbfError::Content.into());
    }
    let request_end = invocation
        .request_offset
        .checked_add(invocation.request_len)
        .ok_or(TradingSbfError::Content)?;
    let request_bytes = request_bank
        .get(invocation.request_offset..request_end)
        .ok_or(TradingSbfError::Content)?;
    if request_bytes.get(..8) != Some(SERIES_CORE_REQUEST_MAGIC_V1.as_slice()) {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let request =
        SeriesCoreRequestV1::decode(request_bytes).map_err(|_| TradingSbfError::Content)?;
    let ticket = request.ticket().ok_or(TradingSbfError::Content)?.to_bytes();
    if request.action() != SeriesCoreActionV1::Consume
        || request.release_set().to_bytes() != parent.release_set
        || request
            .market()
            .is_none_or(|market| market.to_bytes() != parent.market)
        || request.market_generation() != Some(parent.generation)
    {
        return Err(TradingSbfError::Content.into());
    }
    let witness = invocation
        .borrowed_witness
        .ok_or(TradingSbfError::Content)?
        .slice(family_request)
        .map_err(|_| TradingSbfError::Content)?;
    let mut child_data = Vec::with_capacity(
        request_bytes
            .len()
            .checked_add(witness.len())
            .ok_or(TradingSbfError::Content)?,
    );
    child_data.extend_from_slice(request_bytes);
    child_data.extend_from_slice(witness);
    let request_digest = hash(request_bytes).to_bytes();
    let authority_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(parent.release_set).map_err(|_| TradingSbfError::Content)?,
        parent.market,
        ExecutionRoleV1::Trading,
        ticket,
        request_digest,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let (expected_authority, bump) =
        Pubkey::find_program_address(&authority_seeds.as_slices(), program_id);
    let child_accounts = invocation_accounts(invocation, effect_accounts)?;
    if child_accounts
        .first()
        .is_none_or(|account| account.key != &expected_authority)
    {
        return Err(TradingSbfError::Release.into());
    }
    Ok(PreparedCoreInvocationV3 {
        invocation,
        request,
        child_data,
        request_digest,
        authority_seeds,
        bump,
    })
}

fn invocation_accounts<'info>(
    invocation: ResolvedInvocationV3,
    accounts: DowngradedEffectAccountsV3<'_, '_, 'info>,
) -> Result<Vec<AccountInfo<'info>>, ProgramError> {
    let start = usize::from(invocation.fixed_account_start);
    let end = start
        .checked_add(usize::from(invocation.fixed_account_count))
        .ok_or(TradingSbfError::Content)?;
    let mut output = Vec::new();
    accounts.extend_window(
        &mut output,
        start,
        end.checked_sub(start).ok_or(TradingSbfError::Content)?,
    )?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn series_core_is_the_only_current_typed_core_packet() {
        assert_eq!(SERIES_CORE_REQUEST_MAGIC_V1, *b"DCLTCSR1");
        assert_ne!(
            CORE_EXECUTION_DIGEST_DOMAIN_V3,
            b"dclutch:hot-custody-receipt:v3"
        );
    }
}
