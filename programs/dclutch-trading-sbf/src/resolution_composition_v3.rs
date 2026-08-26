//! Family-neutral Resolution CPI execution for EffectProgram V3 routes.
//!
//! The selected EffectProgram owns the exact provider request and account
//! frame. Trading appends only the authenticated borrowed PostUpdate body,
//! signs its release-pinned caller authority, and accepts only an immediate
//! receipt from the current Resolution program. The resulting Source and
//! certificate byte commitments are chained into the common Hot transcript.

extern crate alloc;

use alloc::{vec, vec::Vec};

use dclutch_core_contract::ContentId;
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{ProgramV3, ResolvedInvocationV3, RouteKindV3},
};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_resolution_codec::{
    PROVIDER_EXECUTION_REQUEST_BYTES_V3, PROVIDER_RESOLUTION_TRADING_ACCOUNT_COUNT_V3,
    PROVIDER_RESOLUTION_TRADING_TAIL_START_V3, PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3,
    PROVIDER_UPDATE_LIFECYCLE_BYTES_V3, PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3, ProviderCallerV3,
    ProviderExecutionReceiptV3, ProviderExecutionRequestV3, ProviderUpdateLifecycleV3,
    ProviderUpdateStatusV3,
};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::TradingSbfError;

const RESOLUTION_EXECUTION_DIGEST_DOMAIN_V3: &[u8] = b"dclutch:hot-resolution-receipt:v3";
const CALLER_AUTHORITY_ACCOUNT_V3: usize = 0;
const RESOLVER_ACCOUNT_V3: usize = 1;
const SOURCE_STATE_ACCOUNT_V3: usize = 2;
const CERTIFICATE_ACCOUNT_V3: usize = 3;
const MARKET_ACCOUNT_V3: usize = 4;
const ACTIVATION_ACCOUNT_V3: usize = 5;
const TRADING_PROGRAM_ACCOUNT_V3: usize = 13;
const RESOLUTION_PROGRAM_ACCOUNT_V3: usize = 15;
const LIFECYCLE_ACCOUNT_V3: usize = PROVIDER_RESOLUTION_TRADING_TAIL_START_V3 - 1;
const UPDATE_ACCOUNT_V3: usize = PROVIDER_RESOLUTION_TRADING_TAIL_START_V3;

/// Immutable parent facts every projected provider request must reproduce.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionCompositionParentV3 {
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
    /// Current Trading CapabilityProgramSet content identity.
    pub capability_program_set: [u8; 32],
    /// Action-selected CapabilityProgramV3 content identity.
    pub selected_capability_program: [u8; 32],
    /// Exact activated release-set cache account.
    pub activation_account: [u8; 32],
}

/// Preflight one exact active Resolution invocation without external mutation.
#[allow(clippy::too_many_arguments)]
pub fn preflight_resolution_route_v3(
    program_id: &Pubkey,
    effect: ProgramV3<'_>,
    route_index: u16,
    invocation_index: u32,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    effect_accounts: &[AccountInfo<'_>],
    request_bank: &[u8],
    family_request: &[u8],
    resolution_program: &AccountInfo<'_>,
    parent: ResolutionCompositionParentV3,
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
        resolution_program,
        parent,
    )?;
    Ok(())
}

/// Execute one preflighted Resolution invocation and verify its immediate receipt.
#[allow(clippy::too_many_arguments)]
pub fn execute_resolution_route_v3<'info>(
    program_id: &Pubkey,
    effect: ProgramV3<'_>,
    route_index: u16,
    invocation_index: u32,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    effect_accounts: &[AccountInfo<'info>],
    request_bank: &[u8],
    family_request: &[u8],
    resolution_program: &AccountInfo<'info>,
    parent: ResolutionCompositionParentV3,
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
        family_request,
        resolution_program,
        parent,
    )?;
    let mut child_accounts = invocation_accounts(prepared.invocation, effect_accounts)?;
    let mut metas = Vec::with_capacity(child_accounts.len());
    for (index, account) in child_accounts.iter().enumerate() {
        let signer = index == CALLER_AUTHORITY_ACCOUNT_V3 || account.is_signer;
        metas.push(if account.is_writable {
            AccountMeta::new(*account.key, signer)
        } else {
            AccountMeta::new_readonly(*account.key, signer)
        });
    }
    let instruction = Instruction {
        program_id: *resolution_program.key,
        accounts: metas,
        data: prepared.child_data,
    };
    child_accounts.push(resolution_program.clone());
    let bump_seed = [prepared.bump];
    let [domain, release, market, role, context, digest] = prepared.authority_seeds.as_slices();
    invoke_signed(
        &instruction,
        &child_accounts,
        &[&[domain, release, market, role, context, digest, &bump_seed]],
    )
    .map_err(|_| TradingSbfError::Transition)?;
    let (producer, receipt_bytes) = get_return_data().ok_or(TradingSbfError::Transition)?;
    if producer != *resolution_program.key {
        return Err(TradingSbfError::Transition.into());
    }
    let receipt = ProviderExecutionReceiptV3::decode(&receipt_bytes)
        .map_err(|_| TradingSbfError::Transition)?;
    verify_receipt(prepared.request, prepared.request_digest, receipt)?;
    let lifecycle = decode_lifecycle(
        child_accounts
            .get(LIFECYCLE_ACCOUNT_V3)
            .ok_or(TradingSbfError::Transition)?,
    )?;
    verify_consumed_lifecycle(prepared.request, receipt, lifecycle)?;
    let lifecycle_digest = account_data_digest(
        child_accounts
            .get(LIFECYCLE_ACCOUNT_V3)
            .ok_or(TradingSbfError::Transition)?,
    )?;
    let source_digest = account_data_digest(
        child_accounts
            .get(SOURCE_STATE_ACCOUNT_V3)
            .ok_or(TradingSbfError::Transition)?,
    )?;
    let certificate_digest = account_data_digest(
        child_accounts
            .get(CERTIFICATE_ACCOUNT_V3)
            .ok_or(TradingSbfError::Transition)?,
    )?;
    Ok(hashv(&[
        RESOLUTION_EXECUTION_DIGEST_DOMAIN_V3,
        &route_index.to_le_bytes(),
        &invocation_index.to_le_bytes(),
        &prepared.request_digest,
        &prepared.post_body_digest,
        &receipt_bytes,
        &prepared.lifecycle_pre_digest,
        &lifecycle_digest,
        &source_digest,
        &certificate_digest,
    ])
    .to_bytes())
}

struct PreparedResolutionInvocationV3 {
    invocation: ResolvedInvocationV3,
    request: ProviderExecutionRequestV3,
    child_data: Vec<u8>,
    request_digest: [u8; 32],
    post_body_digest: [u8; 32],
    lifecycle_pre_digest: [u8; 32],
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
    effect_accounts: &[AccountInfo<'_>],
    request_bank: &[u8],
    family_request: &[u8],
    resolution_program: &AccountInfo<'_>,
    parent: ResolutionCompositionParentV3,
) -> Result<PreparedResolutionInvocationV3, ProgramError> {
    validate_parent(program_id, parent)?;
    if !resolution_program.executable
        || resolution_program.is_signer
        || resolution_program.is_writable
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
    if invocation.role != FixedRole::Resolution
        || invocation.kind != RouteKindV3::Once
        || invocation.item.is_some()
        || invocation.repeated_item_count != 0
    {
        return Err(TradingSbfError::Content.into());
    }
    let request_bytes = invocation_request(invocation, request_bank)?;
    if request_bytes.len() != PROVIDER_EXECUTION_REQUEST_BYTES_V3 {
        return Err(TradingSbfError::Content.into());
    }
    let request =
        ProviderExecutionRequestV3::decode(request_bytes).map_err(|_| TradingSbfError::Content)?;
    if request.caller != ProviderCallerV3::Trading
        || request.release_set != parent.release_set
        || request.market != parent.market
        || request.generation != parent.generation
        || request.parent_request_digest != parent.parent_request_digest
        || request.caller_program != parent.trading_program
        || request.capability_program_set != parent.capability_program_set
        || request.selected_capability_program != parent.selected_capability_program
    {
        return Err(TradingSbfError::Content.into());
    }
    let post_body = invocation
        .borrowed_witness
        .ok_or(TradingSbfError::Content)?
        .slice(family_request)
        .map_err(|_| TradingSbfError::Content)?;
    if post_body.is_empty() || hash(post_body).to_bytes() != request.post_params_body_digest {
        return Err(TradingSbfError::Content.into());
    }
    let mut child_data = Vec::with_capacity(
        request_bytes
            .len()
            .checked_add(post_body.len())
            .ok_or(TradingSbfError::Content)?,
    );
    child_data.extend_from_slice(request_bytes);
    child_data.extend_from_slice(post_body);
    let child_accounts = invocation_accounts(invocation, effect_accounts)?;
    if child_accounts.len() != PROVIDER_RESOLUTION_TRADING_ACCOUNT_COUNT_V3
        || child_accounts
            .get(RESOLVER_ACCOUNT_V3)
            .is_none_or(|account| account.key.to_bytes() != request.resolver)
        || child_accounts
            .get(SOURCE_STATE_ACCOUNT_V3)
            .is_none_or(|account| account.key.to_bytes() != request.source_state)
        || child_accounts
            .get(CERTIFICATE_ACCOUNT_V3)
            .is_none_or(|account| account.key.to_bytes() != request.certificate_account)
        || child_accounts
            .get(MARKET_ACCOUNT_V3)
            .is_none_or(|account| account.key.to_bytes() != request.market)
        || child_accounts
            .get(ACTIVATION_ACCOUNT_V3)
            .is_none_or(|account| account.key.to_bytes() != parent.activation_account)
        || child_accounts
            .get(TRADING_PROGRAM_ACCOUNT_V3)
            .is_none_or(|account| account.key != program_id)
        || child_accounts
            .get(RESOLUTION_PROGRAM_ACCOUNT_V3)
            .is_none_or(|account| account.key != resolution_program.key)
        || child_accounts
            .get(UPDATE_ACCOUNT_V3)
            .is_none_or(|account| account.key.to_bytes() != request.update_account)
        || child_accounts
            .iter()
            .filter(|account| account.key == resolution_program.key)
            .count()
            != 1
    {
        return Err(TradingSbfError::Release.into());
    }
    let lifecycle_account = child_accounts
        .get(LIFECYCLE_ACCOUNT_V3)
        .ok_or(TradingSbfError::Content)?;
    let (expected_lifecycle, lifecycle_bump) = Pubkey::find_program_address(
        &[
            PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3,
            &request.update_account,
        ],
        resolution_program.key,
    );
    let lifecycle = decode_lifecycle(lifecycle_account)?;
    let (expected_update_authority, _) = Pubkey::find_program_address(
        &[
            PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3,
            &request.market,
            &request.source_state,
            &request.update_account,
        ],
        resolution_program.key,
    );
    if lifecycle_account.key != &expected_lifecycle
        || lifecycle_account.owner != resolution_program.key
        || lifecycle_account.is_signer
        || !lifecycle_account.is_writable
        || lifecycle_account.executable
        || lifecycle.status != ProviderUpdateStatusV3::Submitted
        || lifecycle.bump != lifecycle_bump
        || lifecycle.generation != request.generation
        || lifecycle.market != request.market
        || lifecycle.source_state != request.source_state
        || lifecycle.source_material != request.source_material
        || lifecycle.provider_release != request.provider_release
        || lifecycle.update_account != request.update_account
        || lifecycle.update_digest != request.expected_update_digest
        || lifecycle.post_body_digest != request.post_params_body_digest
        || lifecycle.provider_submitter != request.provider_submitter
        || lifecycle.release_set != request.release_set
        || lifecycle.registry_program
            != child_accounts
                .get(7)
                .ok_or(TradingSbfError::Content)?
                .key
                .to_bytes()
        || lifecycle.update_authority != expected_update_authority.to_bytes()
    {
        return Err(TradingSbfError::Content.into());
    }
    let authority_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(request.release_set).map_err(|_| TradingSbfError::Content)?,
        request.market,
        ExecutionRoleV1::Trading,
        request.source_state,
        request.parent_request_digest,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let (expected_authority, bump) =
        Pubkey::find_program_address(&authority_seeds.as_slices(), program_id);
    if child_accounts
        .first()
        .is_none_or(|account| account.key != &expected_authority)
    {
        return Err(TradingSbfError::Release.into());
    }
    Ok(PreparedResolutionInvocationV3 {
        invocation,
        request,
        child_data,
        request_digest: hash(request_bytes).to_bytes(),
        post_body_digest: hash(post_body).to_bytes(),
        lifecycle_pre_digest: account_data_digest(lifecycle_account)?,
        authority_seeds,
        bump,
    })
}

fn decode_lifecycle(account: &AccountInfo<'_>) -> Result<ProviderUpdateLifecycleV3, ProgramError> {
    if account.data_len() != PROVIDER_UPDATE_LIFECYCLE_BYTES_V3 {
        return Err(TradingSbfError::Transition.into());
    }
    let bytes = account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    ProviderUpdateLifecycleV3::decode(&bytes).map_err(|_| TradingSbfError::Transition.into())
}

fn verify_consumed_lifecycle(
    request: ProviderExecutionRequestV3,
    receipt: ProviderExecutionReceiptV3,
    lifecycle: ProviderUpdateLifecycleV3,
) -> Result<(), ProgramError> {
    if lifecycle.status != ProviderUpdateStatusV3::Consumed
        || lifecycle.generation != request.generation
        || lifecycle.terminal_sequence != request.terminal_sequence
        || lifecycle.market != request.market
        || lifecycle.source_state != request.source_state
        || lifecycle.source_material != request.source_material
        || lifecycle.provider_release != request.provider_release
        || lifecycle.update_account != request.update_account
        || lifecycle.update_digest != request.expected_update_digest
        || lifecycle.post_body_digest != request.post_params_body_digest
        || lifecycle.provider_submitter != request.provider_submitter
        || lifecycle.release_set != request.release_set
        || lifecycle.provider_evidence != receipt.provider_evidence
        || lifecycle.certificate != request.certificate_account
    {
        Err(TradingSbfError::Transition.into())
    } else {
        Ok(())
    }
}

fn validate_parent(
    program_id: &Pubkey,
    parent: ResolutionCompositionParentV3,
) -> Result<(), ProgramError> {
    if parent.release_set == [0; 32]
        || parent.market == [0; 32]
        || parent.generation == 0
        || parent.parent_request_digest == [0; 32]
        || parent.trading_program != program_id.to_bytes()
        || parent.capability_program_set == [0; 32]
        || parent.selected_capability_program == [0; 32]
        || parent.activation_account == [0; 32]
    {
        Err(TradingSbfError::Content.into())
    } else {
        Ok(())
    }
}

fn verify_receipt(
    request: ProviderExecutionRequestV3,
    request_digest: [u8; 32],
    receipt: ProviderExecutionReceiptV3,
) -> Result<(), ProgramError> {
    if receipt.caller != request.caller
        || receipt.generation != request.generation
        || receipt.terminal_sequence != request.terminal_sequence
        || receipt.request_digest != request_digest
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
        || receipt.capability_program_set != request.capability_program_set
        || receipt.selected_capability_program != request.selected_capability_program
    {
        Err(TradingSbfError::Transition.into())
    } else {
        Ok(())
    }
}

fn account_data_digest(account: &AccountInfo<'_>) -> Result<[u8; 32], ProgramError> {
    let bytes = account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    Ok(hash(&bytes).to_bytes())
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
    let mut output = vec![];
    let fixed_start = usize::from(invocation.fixed_account_start);
    let fixed_end = fixed_start
        .checked_add(usize::from(invocation.fixed_account_count))
        .ok_or(TradingSbfError::Content)?;
    output.extend_from_slice(
        accounts
            .get(fixed_start..fixed_end)
            .ok_or(TradingSbfError::Content)?,
    );
    if invocation.kind != RouteKindV3::Once
        || invocation.item_account_count != 0
        || invocation.repeated_item_count != 0
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_resolution_codec::ProviderSubmitRequestV3;

    fn id(tag: u8) -> [u8; 32] {
        [tag; 32]
    }

    fn request() -> ProviderExecutionRequestV3 {
        ProviderExecutionRequestV3 {
            caller: ProviderCallerV3::Trading,
            generation: 7,
            terminal_sequence: 9,
            market: id(1),
            source_state: id(2),
            certificate_account: id(3),
            source_material: id(4),
            source_spec: id(5),
            product_record: id(6),
            result_domain: id(7),
            provider_release: id(8),
            update_account: id(9),
            expected_update_digest: id(10),
            provider_submitter: id(11),
            resolver: id(12),
            caller_program: id(13),
            release_set: id(14),
            capability_program_set: id(15),
            selected_capability_program: id(16),
            parent_request_digest: id(17),
            post_params_body_digest: id(18),
        }
    }

    fn receipt(
        request: ProviderExecutionRequestV3,
        digest: [u8; 32],
    ) -> ProviderExecutionReceiptV3 {
        ProviderExecutionReceiptV3 {
            caller: request.caller,
            generation: request.generation,
            terminal_sequence: request.terminal_sequence,
            request_digest: digest,
            provider_evidence: id(19),
            update_digest: request.expected_update_digest,
            post_params_body_digest: request.post_params_body_digest,
            market: request.market,
            source_state: request.source_state,
            certificate_account: request.certificate_account,
            source_material: request.source_material,
            product_record: request.product_record,
            result_domain: request.result_domain,
            provider_release: request.provider_release,
            update_account: request.update_account,
            provider_submitter: request.provider_submitter,
            resolver: request.resolver,
            caller_program: request.caller_program,
            release_set: request.release_set,
            capability_program_set: request.capability_program_set,
            selected_capability_program: request.selected_capability_program,
            selector: 0,
            outcome_count: 2,
            result_numerator: 1,
            result_denominator: 1,
            publish_time: 1,
            posted_slot: 1,
            consumed_slot: 1,
        }
    }

    fn submitted_lifecycle(request: ProviderExecutionRequestV3) -> ProviderUpdateLifecycleV3 {
        ProviderUpdateLifecycleV3::submitted(
            ProviderSubmitRequestV3 {
                generation: request.generation,
                reclaim_after_unix_seconds: 2,
                market: request.market,
                source_state: request.source_state,
                lifecycle: id(22),
                source_material: request.source_material,
                provider_release: request.provider_release,
                update_account: request.update_account,
                provider_submitter: request.provider_submitter,
                refund_recipient: id(23),
                release_set: request.release_set,
                registry_program: id(24),
                encoded_vaa: id(25),
                post_body_digest: request.post_params_body_digest,
            },
            1,
            id(26),
            id(24),
            request.expected_update_digest,
            1,
            1,
            1,
            0,
        )
        .expect("submitted lifecycle")
    }

    #[test]
    fn receipt_binds_the_complete_provider_request() {
        let request = request();
        let digest = id(20);
        let receipt = receipt(request, digest);
        assert_eq!(verify_receipt(request, digest, receipt), Ok(()));

        let mut hostile = receipt;
        hostile.request_digest = id(21);
        assert_eq!(
            verify_receipt(request, digest, hostile),
            Err(TradingSbfError::Transition.into())
        );
    }

    #[test]
    fn terminal_receipt_requires_the_exact_consumed_lifecycle() {
        let request = request();
        let receipt = receipt(request, id(20));
        let submitted = submitted_lifecycle(request);
        assert_eq!(
            verify_consumed_lifecycle(request, receipt, submitted),
            Err(TradingSbfError::Transition.into())
        );
        let mut consumed = submitted;
        consumed
            .consume(
                request.terminal_sequence,
                receipt.provider_evidence,
                request.certificate_account,
            )
            .expect("consume lifecycle");
        assert_eq!(
            verify_consumed_lifecycle(request, receipt, consumed),
            Ok(())
        );
        consumed.provider_evidence = id(27);
        assert_eq!(
            verify_consumed_lifecycle(request, receipt, consumed),
            Err(TradingSbfError::Transition.into())
        );
    }
}
