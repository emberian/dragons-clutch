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
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::{
    TradingSbfError,
    child_receipt_v3::{ReceiptDeliveryV3, deliver_receipt_dependency_v3},
    hot_v3::{ChildInvocationBuffersV3, DowngradedEffectAccountsV3},
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
pub fn preflight_core_route_v3<'info>(
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
    frame: &mut Vec<AccountInfo<'info>>,
    wire: &mut Vec<u8>,
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
        frame,
        wire,
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
    buffers: &mut ChildInvocationBuffersV3<'info>,
    core_program: &AccountInfo<'info>,
    parent: CoreCompositionParentV3,
) -> Result<[u8; 32], ProgramError> {
    // `prepare` leaves the authenticated frame and the child wire IN the walk's
    // buffers. It used to build both for its own authority check and then be
    // handed a second copy of each here.
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
        family_request,
        &mut buffers.accounts,
        &mut buffers.data,
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
        &mut buffers.data,
        prior_receipt,
        ReceiptDeliveryV3::ExactSuffix,
    )?;
    buffers.fill_metas()?;
    buffers.push_callee(core_program)?;
    let bump_seed = [prepared.bump];
    let [domain, release, market, role, context, digest] = prepared.authority_seeds.as_slices();
    buffers
        .invoke(
            core_program.key,
            &[&[domain, release, market, role, context, digest, &bump_seed]],
        )
        .map_err(|_| TradingSbfError::Transition)?;
    buffers.capture_return()?;
    if buffers.producer != *core_program.key {
        return Err(TradingSbfError::Transition.into());
    }
    let receipt =
        SeriesCoreAckV1::decode(&buffers.returned).map_err(|_| TradingSbfError::Transition)?;
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
        &hash(&buffers.data).to_bytes(),
        &buffers.returned,
    ])
    .to_bytes())
}

struct PreparedCoreInvocationV3 {
    invocation: ResolvedInvocationV3,
    request: SeriesCoreRequestV1,
    request_digest: [u8; 32],
    authority_seeds: CallerAuthoritySeedsV1,
    bump: u8,
}

#[allow(clippy::too_many_arguments)]
fn prepare<'info>(
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
    frame: &mut Vec<AccountInfo<'info>>,
    wire: &mut Vec<u8>,
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
    wire.clear();
    wire.try_reserve(
        request_bytes
            .len()
            .checked_add(witness.len())
            .ok_or(TradingSbfError::Content)?,
    )
    .map_err(|_| TradingSbfError::Content)?;
    wire.extend_from_slice(request_bytes);
    wire.extend_from_slice(witness);
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
    gather_invocation_accounts(frame, invocation, effect_accounts)?;
    if frame
        .first()
        .is_none_or(|account| account.key != &expected_authority)
    {
        return Err(TradingSbfError::Release.into());
    }
    Ok(PreparedCoreInvocationV3 {
        invocation,
        request,
        request_digest,
        authority_seeds,
        bump,
    })
}

/// Gather this invocation's account window into a caller-owned buffer.
fn gather_invocation_accounts<'info>(
    output: &mut Vec<AccountInfo<'info>>,
    invocation: ResolvedInvocationV3,
    accounts: DowngradedEffectAccountsV3<'_, '_, 'info>,
) -> Result<(), ProgramError> {
    let start = usize::from(invocation.fixed_account_start);
    let end = start
        .checked_add(usize::from(invocation.fixed_account_count))
        .ok_or(TradingSbfError::Content)?;
    accounts.reserve_invocation_frame(output, invocation)?;
    accounts.extend_window(
        output,
        start,
        end.checked_sub(start).ok_or(TradingSbfError::Content)?,
    )
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
