//! Permissionless Core orchestration of one authenticated provider result.
//!
//! Provider submission remains a Resolution action. This route consumes an
//! already submitted update, derives the current Core caller PDA, invokes the
//! Registry-selected Resolution program, checks its immediate receipt and
//! terminal poststate, then admits that exact certificate into the Market.

use alloc::{boxed::Box, vec::Vec};

use dclutch_market_core_codec::{
    Action, CoreState, Phase, Product, Readiness, Request, Role, STATE_BYTES, TerminalReceipt,
    admit_terminal,
};
use dclutch_product_runtime_v2_svm_reader::{FinalizedRecordFrameV2, ProductRuntimeFrameV2};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_resolution_codec::{
    PROVIDER_EXECUTION_RECEIPT_BYTES_V3, PROVIDER_EXECUTION_REQUEST_BYTES_V3,
    PROVIDER_RESOLUTION_CORE_ACCOUNT_COUNT_V3, PROVIDER_RESOLUTION_CORE_TAIL_START_V3,
    PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3, PROVIDER_UPDATE_LIFECYCLE_BYTES_V3,
    PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3, ProviderCallerV3, ProviderExecutionReceiptV3,
    ProviderExecutionRequestV3, ProviderUpdateLifecycleV3, ProviderUpdateStatusV3,
    RESOLUTION_CERTIFICATE_BYTES_V2, RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
    ResolutionCertificateKindV2, ResolutionCertificateV2,
};
use dclutch_source_contract::{
    SOURCE_RESOLUTION_STATE_BYTES_V2, SourceResolutionPhaseV1, SourceResolutionRouteV1,
    SourceResolutionStateV2,
};
use solana_program::{
    account_info::AccountInfo,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{system_program, sysvar};

use crate::{
    CoreSbfError,
    fixed_role::{authenticate_market, persist_state, read_market_bytes},
    frame::require_distinct,
    product_runtime_v2::{authenticate_selected_runtime_v2, project_core_product_v2},
    release::{RoleDeploymentAccounts, authenticate_roles},
};

/// Exact account count shared with the Resolution Core-caller profile.
pub const EXECUTE_PROVIDER_ACCOUNT_COUNT_V3: usize = PROVIDER_RESOLUTION_CORE_ACCOUNT_COUNT_V3;
/// Fixed Core request plus fixed provider request before the borrowed provider body.
pub const EXECUTE_PROVIDER_PREFIX_BYTES_V3: usize =
    dclutch_market_core_codec::REQUEST_BYTES + PROVIDER_EXECUTION_REQUEST_BYTES_V3;

const CALLER_AUTHORITY: usize = 0;
const RESOLVER: usize = 1;
const SOURCE_STATE: usize = 2;
const CERTIFICATE: usize = 3;
const MARKET: usize = 4;
const ACTIVATION: usize = 5;
const REGISTRY: usize = 7;
const CORE_PROGRAM: usize = 11;
const CORE_PROGRAMDATA: usize = 12;
const RESOLUTION_PROGRAM: usize = 15;
const RESOLUTION_PROGRAMDATA: usize = 16;
const PRODUCT: usize = 31;
const RESULT_DOMAIN: usize = 33;
const PORTFOLIO: usize = 35;
const LIFECYCLE: usize = PROVIDER_RESOLUTION_CORE_TAIL_START_V3 - 1;
const UPDATE: usize = PROVIDER_RESOLUTION_CORE_TAIL_START_V3;
const RENT: usize = PROVIDER_RESOLUTION_CORE_TAIL_START_V3 + 7;
const SYSTEM: usize = PROVIDER_RESOLUTION_CORE_TAIL_START_V3 + 8;

/// Execute one provider request and commit the corresponding terminal Market last.
#[inline(never)]
pub(crate) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: Request,
    request_bytes: &[u8],
    provider_data: &[u8],
) -> Result<(), ProgramError> {
    let (provider_request_bytes, post_body) = provider_data
        .split_at_checked(PROVIDER_EXECUTION_REQUEST_BYTES_V3)
        .ok_or(CoreSbfError::Instruction)?;
    if post_body.is_empty() {
        return Err(CoreSbfError::Instruction.into());
    }
    let provider = boxed_provider_request(provider_request_bytes)?;
    validate_outer_frame(program_id, accounts)?;

    let state_bytes = Box::new(read_market_bytes(program_id, account(accounts, MARKET)?)?);
    let state =
        Box::new(CoreState::decode(state_bytes.as_ref()).map_err(|_| CoreSbfError::Market)?);
    authenticate_market(program_id, account(accounts, MARKET)?, *state, request)?;
    authenticate_parent(
        program_id,
        accounts,
        request,
        request_bytes,
        *state,
        *provider,
    )?;

    let admissions = authenticate_roles(
        account(accounts, ACTIVATION)?,
        account(accounts, REGISTRY)?,
        state.identity.registry_program,
        state.identity.selected_release_set.to_bytes(),
        &[
            RoleDeploymentAccounts::new(
                Role::Core,
                account(accounts, CORE_PROGRAM)?,
                account(accounts, CORE_PROGRAMDATA)?,
            ),
            RoleDeploymentAccounts::new(
                Role::Resolution,
                account(accounts, RESOLUTION_PROGRAM)?,
                account(accounts, RESOLUTION_PROGRAMDATA)?,
            ),
        ],
    )?;
    let resolution_admission = Box::new(admissions.admission(Role::Resolution)?);

    let rent = read_rent(account(accounts, RENT)?)?;
    let product = Box::new(authenticate_product(accounts, *state, *provider, &rent)?);

    invoke_resolution(
        program_id,
        accounts,
        provider_request_bytes,
        post_body,
        *provider,
    )?;
    require_unchanged_market(account(accounts, MARKET)?, &state_bytes)?;
    let receipt = boxed_immediate_receipt(account(accounts, RESOLUTION_PROGRAM)?, *provider)?;
    authenticate_terminal_poststate(accounts, *state, *provider, *receipt, *product, &rent)?;

    let mut candidate = Box::new(*state);
    let semantic_request =
        Request::administrative(Action::AdmitTerminal, request.generation, request.market);
    admit_terminal(
        semantic_request,
        &mut candidate,
        *resolution_admission,
        *product,
        true,
        TerminalReceipt {
            receipt_id: identity(provider.certificate_account)?,
            market_id: state.identity.market_id,
            resolution_policy: state.identity.resolution_policy,
            product_id: state.identity.product_id,
            generation: state.identity.generation,
            selector: receipt.selector,
            authenticated: true,
        },
    )
    .map_err(|_| CoreSbfError::Transition)?;
    persist_state(account(accounts, MARKET)?, *candidate)
}

#[inline(never)]
fn boxed_provider_request(bytes: &[u8]) -> Result<Box<ProviderExecutionRequestV3>, CoreSbfError> {
    Ok(Box::new(
        ProviderExecutionRequestV3::decode(bytes).map_err(|_| CoreSbfError::Instruction)?,
    ))
}

#[inline(never)]
fn boxed_immediate_receipt(
    resolution_program: &AccountInfo<'_>,
    request: ProviderExecutionRequestV3,
) -> Result<Box<ProviderExecutionReceiptV3>, CoreSbfError> {
    Ok(Box::new(immediate_receipt(resolution_program, request)?))
}

#[inline(never)]
fn authenticate_parent(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: Request,
    request_bytes: &[u8],
    state: CoreState,
    provider: ProviderExecutionRequestV3,
) -> Result<(), CoreSbfError> {
    if request.action != Action::ExecuteProvider
        || state.phase != Phase::Open
        || state.readiness != Readiness::Consumed
        || provider.caller != ProviderCallerV3::Core
        || provider.generation != request.generation
        || provider.market != account(accounts, MARKET)?.key.to_bytes()
        || provider.market != request.market.to_bytes()
        || provider.source_state != account(accounts, SOURCE_STATE)?.key.to_bytes()
        || provider.certificate_account != account(accounts, CERTIFICATE)?.key.to_bytes()
        || provider.update_account != account(accounts, UPDATE)?.key.to_bytes()
        || provider.resolver != account(accounts, RESOLVER)?.key.to_bytes()
        || provider.caller_program != program_id.to_bytes()
        || provider.release_set != state.identity.selected_release_set.to_bytes()
        || provider.source_material != state.identity.resolution_policy.to_bytes()
        || provider.product_record != state.identity.product_record.to_bytes()
        || provider.parent_request_digest != hash(request_bytes).to_bytes()
    {
        return Err(CoreSbfError::Reference);
    }
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        provider.release_set,
        provider.market,
        ExecutionRoleV1::Core,
        provider.source_state,
        provider.parent_request_digest,
    )
    .map_err(|_| CoreSbfError::CallerAuthority)?;
    if Pubkey::find_program_address(&seeds.as_slices(), program_id).0
        != *account(accounts, CALLER_AUTHORITY)?.key
    {
        return Err(CoreSbfError::CallerAuthority);
    }
    Ok(())
}

#[inline(never)]
fn invoke_resolution<'info>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'info>],
    request_bytes: &[u8],
    post_body: &[u8],
    request: ProviderExecutionRequestV3,
) -> Result<(), ProgramError> {
    let mut data = Vec::with_capacity(request_bytes.len().saturating_add(post_body.len()));
    data.extend_from_slice(request_bytes);
    data.extend_from_slice(post_body);
    let mut metas = Vec::with_capacity(accounts.len());
    for (index, value) in accounts.iter().enumerate() {
        let signer = index == CALLER_AUTHORITY || value.is_signer;
        let writable = index != MARKET && value.is_writable;
        metas.push(if writable {
            AccountMeta::new(*value.key, signer)
        } else {
            AccountMeta::new_readonly(*value.key, signer)
        });
    }
    let resolution_program = account(accounts, RESOLUTION_PROGRAM)?;
    let instruction = Instruction {
        program_id: *resolution_program.key,
        accounts: metas,
        data,
    };
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        request.release_set,
        request.market,
        ExecutionRoleV1::Core,
        request.source_state,
        request.parent_request_digest,
    )
    .map_err(|_| CoreSbfError::CallerAuthority)?;
    let (_, bump) = Pubkey::find_program_address(&seeds.as_slices(), program_id);
    let [domain, release, market, role, context, digest] = seeds.as_slices();
    let bump_seed = [bump];
    let signer: [&[u8]; 7] = [domain, release, market, role, context, digest, &bump_seed];
    let mut infos = Vec::with_capacity(accounts.len().saturating_add(1));
    infos.extend(accounts.iter().cloned());
    infos.push(resolution_program.clone());
    invoke_signed(&instruction, &infos, &[&signer]).map_err(|_| CoreSbfError::ChildCpi.into())
}

fn immediate_receipt(
    resolution_program: &AccountInfo<'_>,
    request: ProviderExecutionRequestV3,
) -> Result<ProviderExecutionReceiptV3, CoreSbfError> {
    let (producer, bytes) = get_return_data().ok_or(CoreSbfError::ChildAck)?;
    if producer != *resolution_program.key || bytes.len() != PROVIDER_EXECUTION_RECEIPT_BYTES_V3 {
        return Err(CoreSbfError::ChildAck);
    }
    let receipt = ProviderExecutionReceiptV3::decode(&bytes).map_err(|_| CoreSbfError::ChildAck)?;
    if receipt.caller != request.caller
        || receipt.generation != request.generation
        || receipt.terminal_sequence != request.terminal_sequence
        || receipt.request_digest
            != hash(&request.to_bytes().map_err(|_| CoreSbfError::Instruction)?).to_bytes()
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
    {
        return Err(CoreSbfError::ChildAck);
    }
    Ok(receipt)
}

fn authenticate_product(
    accounts: &[AccountInfo<'_>],
    state: CoreState,
    request: ProviderExecutionRequestV3,
    rent: &Rent,
) -> Result<Product, CoreSbfError> {
    let runtime = authenticate_selected_runtime_v2(
        account(accounts, REGISTRY)?.key,
        rent,
        state.identity.product_record.to_bytes(),
        ProductRuntimeFrameV2 {
            product: record_frame(accounts, PRODUCT)?,
            result_domain: record_frame(accounts, RESULT_DOMAIN)?,
            portfolio: record_frame(accounts, PORTFOLIO)?,
        },
    )?;
    let product = project_core_product_v2(runtime)?;
    if product.product_record.to_bytes() != request.product_record
        || product.product_id != state.identity.product_id
        || product.result_domain.to_bytes() != request.result_domain
    {
        return Err(CoreSbfError::Reference);
    }
    Ok(product)
}

fn authenticate_terminal_poststate(
    accounts: &[AccountInfo<'_>],
    state: CoreState,
    request: ProviderExecutionRequestV3,
    receipt: ProviderExecutionReceiptV3,
    product: Product,
    rent: &Rent,
) -> Result<(), CoreSbfError> {
    if receipt.outcome_count != product.outcome_count || receipt.selector >= product.outcome_count {
        return Err(CoreSbfError::ChildAck);
    }
    let resolution_program = account(accounts, RESOLUTION_PROGRAM)?.key;
    let source_account = account(accounts, SOURCE_STATE)?;
    let source_bytes =
        read_exact::<SOURCE_RESOLUTION_STATE_BYTES_V2>(source_account, CoreSbfError::ChildAck)?;
    if source_account.owner != resolution_program || source_account.executable {
        return Err(CoreSbfError::ChildAck);
    }
    let source =
        SourceResolutionStateV2::decode(&source_bytes).map_err(|_| CoreSbfError::ChildAck)?;
    let source_seeds = source.pda_seeds();
    let source_bump = [source_seeds.bump()];
    if Pubkey::create_program_address(
        &[
            source_seeds.domain(),
            &source_seeds.market(),
            &source_seeds.generation_le(),
            &source_bump,
        ],
        resolution_program,
    )
    .map_err(|_| CoreSbfError::ChildAck)?
        != *source_account.key
        || source.phase() != SourceResolutionPhaseV1::Resolved
        || source.market() != request.market
        || source.generation() != request.generation
        || source.material_id().to_bytes() != request.source_material
    {
        return Err(CoreSbfError::ChildAck);
    }
    let decision = source
        .decision(product.outcome_count)
        .map_err(|_| CoreSbfError::ChildAck)?;
    if decision.route() != SourceResolutionRouteV1::Primary
        || decision.selector() != receipt.selector
        || decision.outcome_count() != receipt.outcome_count
        || decision.resolution_evidence_id().to_bytes() != receipt.provider_evidence
        || decision.terminal_sequence() != request.terminal_sequence
    {
        return Err(CoreSbfError::ChildAck);
    }
    authenticate_lifecycle(accounts, request, receipt, rent)?;
    authenticate_certificate(accounts, state, request, receipt, product, rent)
}

fn authenticate_lifecycle(
    accounts: &[AccountInfo<'_>],
    request: ProviderExecutionRequestV3,
    receipt: ProviderExecutionReceiptV3,
    rent: &Rent,
) -> Result<(), CoreSbfError> {
    let resolution_program = account(accounts, RESOLUTION_PROGRAM)?.key;
    let lifecycle_account = account(accounts, LIFECYCLE)?;
    let lifecycle_bytes = read_exact::<PROVIDER_UPDATE_LIFECYCLE_BYTES_V3>(
        lifecycle_account,
        CoreSbfError::ChildAck,
    )?;
    let lifecycle =
        ProviderUpdateLifecycleV3::decode(&lifecycle_bytes).map_err(|_| CoreSbfError::ChildAck)?;
    let expected = Pubkey::find_program_address(
        &[
            PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3,
            &request.update_account,
        ],
        resolution_program,
    )
    .0;
    let authority = Pubkey::find_program_address(
        &[
            PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3,
            &request.market,
            &request.source_state,
            &request.update_account,
        ],
        resolution_program,
    )
    .0;
    if lifecycle_account.key != &expected
        || lifecycle_account.owner != resolution_program
        || lifecycle_account.executable
        || !rent.is_exempt(lifecycle_account.lamports(), lifecycle_account.data_len())
        || lifecycle.status != ProviderUpdateStatusV3::Consumed
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
        || lifecycle.update_authority != authority.to_bytes()
        || lifecycle.release_set != request.release_set
        || lifecycle.registry_program != account(accounts, REGISTRY)?.key.to_bytes()
        || lifecycle.provider_evidence != receipt.provider_evidence
        || lifecycle.certificate != request.certificate_account
        || lifecycle.publish_time != receipt.publish_time
        || lifecycle.posted_slot != receipt.posted_slot
    {
        return Err(CoreSbfError::ChildAck);
    }
    Ok(())
}

fn authenticate_certificate(
    accounts: &[AccountInfo<'_>],
    state: CoreState,
    request: ProviderExecutionRequestV3,
    receipt: ProviderExecutionReceiptV3,
    product: Product,
    rent: &Rent,
) -> Result<(), CoreSbfError> {
    let resolution_program = account(accounts, RESOLUTION_PROGRAM)?.key;
    let certificate_account = account(accounts, CERTIFICATE)?;
    let bytes =
        read_exact::<RESOLUTION_CERTIFICATE_BYTES_V2>(certificate_account, CoreSbfError::ChildAck)?;
    let certificate =
        ResolutionCertificateV2::decode(&bytes).map_err(|_| CoreSbfError::ChildAck)?;
    let kind = [1_u8];
    let sequence = request.terminal_sequence.to_le_bytes();
    let expected = Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            account(accounts, SOURCE_STATE)?.key.as_ref(),
            &kind,
            &sequence,
        ],
        resolution_program,
    )
    .0;
    let observed_at = u64::try_from(receipt.publish_time).map_err(|_| CoreSbfError::Arithmetic)?;
    if certificate_account.key != &expected
        || certificate_account.owner != resolution_program
        || certificate_account.executable
        || !rent.is_exempt(
            certificate_account.lamports(),
            certificate_account.data_len(),
        )
        || certificate.kind != ResolutionCertificateKindV2::ResolutionSuccess
        || certificate.market != request.market
        || certificate.route != request.provider_release
        || certificate.source_material != request.source_material
        || certificate.product_record_digest != state.identity.product_record.to_bytes()
        || certificate.provider_evidence != receipt.provider_evidence
        || certificate.funding_allocation != [0; 32]
        || certificate.receipt_account != request.certificate_account
        || certificate.generation != request.generation
        || certificate.attempt_index != 0
        || certificate.schedule_index != 0
        || certificate.selector != receipt.selector
        || certificate.work_paid != 0
        || certificate.funding_remaining != 0
        || certificate.result_numerator != receipt.result_numerator
        || certificate.result_denominator != receipt.result_denominator
        || certificate.observed_at != observed_at
        || certificate
            .validate_terminal_product(request.product_record, product.outcome_count)
            .is_err()
    {
        return Err(CoreSbfError::ChildAck);
    }
    Ok(())
}

fn validate_outer_frame(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
) -> Result<(), CoreSbfError> {
    if accounts.len() != EXECUTE_PROVIDER_ACCOUNT_COUNT_V3 {
        return Err(CoreSbfError::AccountFrame);
    }
    require_distinct(accounts)?;
    for (index, value) in accounts.iter().enumerate() {
        let signer = index == RESOLVER;
        let writable = matches!(index, SOURCE_STATE | CERTIFICATE | MARKET | LIFECYCLE);
        let executable = matches!(
            index,
            REGISTRY | CORE_PROGRAM | 13 | RESOLUTION_PROGRAM | 39 | 42 | SYSTEM
        );
        if value.is_signer != signer
            || value.is_writable != writable
            || value.executable != executable
        {
            return Err(CoreSbfError::AccountFrame);
        }
    }
    if account(accounts, CORE_PROGRAM)?.key != program_id
        || account(accounts, SYSTEM)?.key != &system_program::ID
    {
        return Err(CoreSbfError::AccountFrame);
    }
    Ok(())
}

fn require_unchanged_market(
    market: &AccountInfo<'_>,
    expected: &[u8; STATE_BYTES],
) -> Result<(), CoreSbfError> {
    let data = market.try_borrow_data().map_err(|_| CoreSbfError::Market)?;
    if data.as_ref() == expected {
        Ok(())
    } else {
        Err(CoreSbfError::Market)
    }
}

fn read_rent(account: &AccountInfo<'_>) -> Result<Rent, CoreSbfError> {
    if account.key != &sysvar::rent::ID
        || account.owner != &sysvar::ID
        || account.is_signer
        || account.is_writable
        || account.executable
    {
        return Err(CoreSbfError::AccountFrame);
    }
    Rent::from_account_info(account).map_err(|_| CoreSbfError::AccountFrame)
}

fn record_frame<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<FinalizedRecordFrameV2<'accounts, 'info>, CoreSbfError> {
    Ok(FinalizedRecordFrameV2 {
        raw: account(accounts, index)?,
        staging: account(accounts, index + 1)?,
    })
}

fn read_exact<const N: usize>(
    account: &AccountInfo<'_>,
    error: CoreSbfError,
) -> Result<[u8; N], CoreSbfError> {
    let data = account.try_borrow_data().map_err(|_| error)?;
    data.as_ref().try_into().map_err(|_| error)
}

fn identity(bytes: [u8; 32]) -> Result<dclutch_market_core_codec::Identity, CoreSbfError> {
    dclutch_market_core_codec::Identity::new(bytes).map_err(|_| CoreSbfError::Reference)
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, CoreSbfError> {
    accounts.get(index).ok_or(CoreSbfError::AccountFrame)
}
