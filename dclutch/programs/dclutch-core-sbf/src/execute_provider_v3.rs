//! Permissionless Core orchestration of one authenticated provider result.
//!
//! Provider submission remains a Resolution action. This route consumes an
//! already submitted update, derives the current Core caller PDA, invokes the
//! Registry-selected Resolution program, checks its immediate receipt and
//! terminal poststate. A later standalone Core `AdmitTerminal` consumes that
//! durable certificate and commits the Market transition.

use alloc::{boxed::Box, vec::Vec};

use dclutch_market::capability_manifest::funding::funded_rent_persists_v1;
use dclutch_market::{
    Action, CoreState, MarketAdmissionV1, Phase, Product, Readiness, Request, Role, STATE_BYTES,
};
use dclutch_product::svm_reader::{FinalizedRecordFrameV2, ProductRuntimeFrameV2};
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_source::resolution::{
    PROVIDER_EXECUTION_RECEIPT_BYTES_V3, PROVIDER_EXECUTION_REQUEST_BYTES_V3,
    PROVIDER_RESOLUTION_CORE_ACCOUNT_COUNT_V3, PROVIDER_RESOLUTION_CORE_TAIL_START_V3,
    PROVIDER_RESOLUTION_RECOVERY_TAIL_ACCOUNTS_V3, PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3,
    PROVIDER_UPDATE_LIFECYCLE_BYTES_V3, PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3, ProviderCallerV3,
    ProviderExecutionReceiptV3, ProviderExecutionRequestV3, ProviderUpdateLifecycleV3,
    ProviderUpdateStatusV3, RESOLUTION_CERTIFICATE_BYTES_V2, RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
    ResolutionCertificateKindV2, ResolutionCertificateV2,
};
use dclutch_source::{
    PROVIDER_RELEASE_SCHEMA_ID_V1, ProviderReleaseV1, RECOVERY_POLICY_SCHEMA_ID_V2,
    RecoveryPolicyV2, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3, SOURCE_RESOLUTION_STATE_BYTES_V2,
    SOURCE_SPEC_SCHEMA_ID_V1, SourceMaterialV3, SourceResolutionPhaseV1, SourceResolutionRouteV1,
    SourceResolutionStateV2, SourceSpecV1,
};
use solana_program::{
    account_info::AccountInfo,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;

use crate::{
    CoreSbfError,
    fixed_role::{authenticate_market, read_market_bytes},
    frame::require_distinct,
    product_runtime_v2::{authenticate_selected_runtime_v2, project_core_product_v2},
    release::{RoleDeploymentAccounts, authenticate_roles},
};

/// Market prestates in which Core composes one provider execution.
///
/// One prestate. The Market has opened, and its Resolution Fund readiness has
/// been consumed by that opening, which is what makes a resolver's update the
/// only remaining source of a terminal result.
pub const EXECUTE_PROVIDER_ADMISSIBLE_PRESTATES_V1: MarketAdmissionV1 =
    MarketAdmissionV1::prestates(&[(Phase::Open, Readiness::Consumed)]);

/// Exact account count shared with the Resolution Core-caller profile.
pub const EXECUTE_PROVIDER_ACCOUNT_COUNT_V3: usize = PROVIDER_RESOLUTION_CORE_ACCOUNT_COUNT_V3;
/// The same count for a capture answering on a rung above the primary, which
/// brings the `RecoveryPolicyV2` record pair at the tail.
///
/// Core reads the same declared rung the child does and arrives at the same
/// width independently. That is not redundancy: Core forwards this frame to
/// Resolution by CPI, so a Core that admitted only one width would make the
/// ladder's capture route unreachable through its own caller, and a Core that
/// admitted either width for either rung would forward two accounts nothing
/// checked.
pub const EXECUTE_PROVIDER_RECOVERY_ACCOUNT_COUNT_V3: usize =
    EXECUTE_PROVIDER_ACCOUNT_COUNT_V3 + PROVIDER_RESOLUTION_RECOVERY_TAIL_ACCOUNTS_V3;
/// Fixed Core request plus fixed provider request before the borrowed provider body.
pub const EXECUTE_PROVIDER_PREFIX_BYTES_V3: usize =
    dclutch_market::REQUEST_BYTES + PROVIDER_EXECUTION_REQUEST_BYTES_V3;

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
const SYSTEM: usize = PROVIDER_RESOLUTION_CORE_TAIL_START_V3 + 8;

/// Invoke one provider request under Core's request-bound caller PDA.
///
/// Resolution persists the durable Source, certificate, and provider lifecycle
/// in this transaction. The Market remains read-only and Open so the separate
/// `AdmitTerminal` action can authenticate that finalized poststate and commit
/// the terminal Core transition without another Resolution CPI.
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
    validate_outer_frame(program_id, accounts, provider.source_index)?;

    let state_bytes = Box::new(read_market_bytes(program_id, account(accounts, MARKET)?)?);
    let state =
        Box::new(CoreState::decode(state_bytes.as_ref()).map_err(|_| CoreSbfError::Market)?);
    authenticate_market(program_id, account(accounts, MARKET)?, *state, request)?;
    let caller_bump = authenticate_parent(
        program_id,
        accounts,
        request,
        request_bytes,
        &state,
        &provider,
    )?;

    authenticate_roles(
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

    let product = Box::new(authenticate_product(accounts, &state, &provider)?);

    let certificate_route = authenticate_provider_route(accounts, &provider)?;

    invoke_resolution(
        accounts,
        provider_request_bytes,
        post_body,
        &provider,
        caller_bump,
    )?;
    require_unchanged_market(account(accounts, MARKET)?, &state_bytes)?;
    let receipt = boxed_immediate_receipt(account(accounts, RESOLUTION_PROGRAM)?, &provider)?;
    authenticate_terminal_poststate(
        accounts,
        &state,
        &provider,
        &receipt,
        &product,
        certificate_route,
    )?;
    Ok(())
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
    request: &ProviderExecutionRequestV3,
) -> Result<Box<ProviderExecutionReceiptV3>, CoreSbfError> {
    Ok(Box::new(immediate_receipt(resolution_program, request)?))
}

#[inline(never)]
fn authenticate_parent(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: Request,
    request_bytes: &[u8],
    state: &CoreState,
    provider: &ProviderExecutionRequestV3,
) -> Result<u8, CoreSbfError> {
    if request.action != Action::ExecuteProvider
        || !EXECUTE_PROVIDER_ADMISSIBLE_PRESTATES_V1.admits(state.phase, state.readiness)
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
    let (expected, bump) = Pubkey::find_program_address(&seeds.as_slices(), program_id);
    if expected != *account(accounts, CALLER_AUTHORITY)?.key {
        return Err(CoreSbfError::CallerAuthority);
    }
    Ok(bump)
}

#[inline(never)]
fn invoke_resolution<'info>(
    accounts: &[AccountInfo<'info>],
    request_bytes: &[u8],
    post_body: &[u8],
    request: &ProviderExecutionRequestV3,
    caller_bump: u8,
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
    let [domain, release, market, role, context, digest] = seeds.as_slices();
    let bump_seed = [caller_bump];
    let signer: [&[u8]; 7] = [domain, release, market, role, context, digest, &bump_seed];
    let mut infos = Vec::with_capacity(accounts.len().saturating_add(1));
    infos.extend(accounts.iter().cloned());
    infos.push(resolution_program.clone());
    invoke_signed(&instruction, &infos, &[&signer]).map_err(|_| CoreSbfError::ChildCpi.into())
}

fn immediate_receipt(
    resolution_program: &AccountInfo<'_>,
    request: &ProviderExecutionRequestV3,
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
    state: &CoreState,
    request: &ProviderExecutionRequestV3,
) -> Result<Product, CoreSbfError> {
    let runtime = authenticate_selected_runtime_v2(
        account(accounts, REGISTRY)?.key,
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

/// Select the certificate route from immutable records and the Source prestate.
/// The Source clears its active attempt on resolution, so this join occurs
/// before CPI; the child cannot substitute the route it later certifies.
#[inline(never)]
fn authenticate_provider_route(
    accounts: &[AccountInfo<'_>],
    request: &ProviderExecutionRequestV3,
) -> Result<[u8; 32], CoreSbfError> {
    let material = with_source_record(
        accounts,
        17,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        request.source_material,
        SourceMaterialV3::decode,
    )?;
    let source_spec = with_source_record(
        accounts,
        19,
        SOURCE_SPEC_SCHEMA_ID_V1,
        request.source_spec,
        SourceSpecV1::decode,
    )?;
    let route = source_spec.provider_release_id().to_bytes();
    let provider = with_source_record(
        accounts,
        21,
        PROVIDER_RELEASE_SCHEMA_ID_V1,
        route,
        ProviderReleaseV1::decode,
    )?;
    if material.product_record_digest().to_bytes() != request.product_record
        || provider.provider_deployment_release_id().to_bytes() != request.provider_release
    {
        return Err(CoreSbfError::Reference);
    }
    let source_account = account(accounts, SOURCE_STATE)?;
    let source_bytes =
        read_exact::<SOURCE_RESOLUTION_STATE_BYTES_V2>(source_account, CoreSbfError::Reference)?;
    let source =
        SourceResolutionStateV2::decode(&source_bytes).map_err(|_| CoreSbfError::Reference)?;
    let resolution = account(accounts, RESOLUTION_PROGRAM)?.key;
    let seeds = source.pda_seeds();
    let bump = [seeds.bump()];
    let expected = Pubkey::create_program_address(
        &[
            seeds.domain(),
            &seeds.market(),
            &seeds.generation_le(),
            &bump,
        ],
        resolution,
    )
    .map_err(|_| CoreSbfError::Reference)?;
    if source_account.key != &expected
        || source_account.owner != resolution
        || source_account.executable
        || source.market() != request.market
        || source.generation() != request.generation
        || source.material_id().to_bytes() != request.source_material
        || source.next_terminal_sequence().ok() != Some(request.terminal_sequence)
    {
        return Err(CoreSbfError::Reference);
    }
    match (request.source_index, source.phase()) {
        (0, SourceResolutionPhaseV1::Primary) if material.ensemble().is_single() => {
            if material.primary_source_spec().to_bytes() != request.source_spec {
                return Err(CoreSbfError::Reference);
            }
        }
        (index, SourceResolutionPhaseV1::Recovery) if index != 0 => {
            let attempt_index = index.checked_sub(1).ok_or(CoreSbfError::Reference)?;
            if source.active_attempt() != attempt_index {
                return Err(CoreSbfError::Reference);
            }
            authenticate_recovery_provider_route(
                accounts,
                request,
                material,
                source_spec,
                attempt_index,
            )?;
        }
        _ => return Err(CoreSbfError::Reference),
    }
    Ok(route)
}

#[inline(never)]
fn authenticate_recovery_provider_route(
    accounts: &[AccountInfo<'_>],
    request: &ProviderExecutionRequestV3,
    material: SourceMaterialV3,
    source: SourceSpecV1,
    attempt_index: u8,
) -> Result<(), CoreSbfError> {
    let policy_id = material.recovery_policy().ok_or(CoreSbfError::Reference)?;
    let policy = with_source_record(
        accounts,
        EXECUTE_PROVIDER_ACCOUNT_COUNT_V3,
        RECOVERY_POLICY_SCHEMA_ID_V2,
        policy_id.to_bytes(),
        RecoveryPolicyV2::decode,
    )?;
    let attempt = policy
        .attempt(attempt_index)
        .map_err(|_| CoreSbfError::Reference)?;
    if attempt.source_spec_id().to_bytes() != request.source_spec
        || attempt.provider_release_id() != source.provider_release_id()
    {
        return Err(CoreSbfError::Reference);
    }
    policy
        .validate_capacity_profile(source.capacity_profile_id())
        .map_err(|_| CoreSbfError::Reference)
}

fn with_source_record<T>(
    accounts: &[AccountInfo<'_>],
    index: usize,
    schema: [u8; 32],
    digest: [u8; 32],
    decode: impl FnOnce(&[u8]) -> dclutch_source::Result<T>,
) -> Result<T, CoreSbfError> {
    let raw = account(accounts, index)?;
    let data = raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    let bytes = crate::records::authenticate_finalized_record(
        account(accounts, REGISTRY)?.key,
        raw,
        account(accounts, index + 1)?,
        schema,
        digest,
        &data,
    )?;
    decode(bytes).map_err(|_| CoreSbfError::Reference)
}

fn authenticate_certificate_route(
    observed: [u8; 32],
    authenticated_source_provider: [u8; 32],
) -> Result<(), CoreSbfError> {
    if observed != authenticated_source_provider {
        solana_program::msg!("provider certificate differs from selected Source ProviderRelease");
        return Err(CoreSbfError::ChildAck);
    }
    Ok(())
}

fn authenticate_terminal_poststate(
    accounts: &[AccountInfo<'_>],
    state: &CoreState,
    request: &ProviderExecutionRequestV3,
    receipt: &ProviderExecutionReceiptV3,
    product: &Product,
    certificate_route: [u8; 32],
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
    // WHICH ROUTE ANSWERED, and it has to be the one the request asked on.
    // A capture on rung zero is the primary route and a capture on any rung
    // above it is the recovery route, so Core admits exactly one of the two per
    // request rather than admitting either -- a receipt that claimed the
    // primary while the Source recorded a recovery terminal would otherwise be
    // accepted, and the market's own answer would be attributed to a feed that
    // did not give it.
    let expected_route = if request.source_index == 0 {
        SourceResolutionRouteV1::Primary
    } else {
        SourceResolutionRouteV1::Recovery
    };
    if decision.route() != expected_route
        || decision.selector() != receipt.selector
        || decision.outcome_count() != receipt.outcome_count
        || decision.resolution_evidence_id().to_bytes() != receipt.provider_evidence
        || decision.terminal_sequence() != request.terminal_sequence
    {
        return Err(CoreSbfError::ChildAck);
    }
    authenticate_lifecycle(accounts, request, receipt)?;
    authenticate_certificate(
        accounts,
        state.identity.product_record.to_bytes(),
        request,
        receipt,
        product.outcome_count,
        certificate_route,
    )
}

fn authenticate_lifecycle(
    accounts: &[AccountInfo<'_>],
    request: &ProviderExecutionRequestV3,
    receipt: &ProviderExecutionReceiptV3,
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
        || !funded_rent_persists_v1(lifecycle_account.lamports())
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
    product_record: [u8; 32],
    request: &ProviderExecutionRequestV3,
    receipt: &ProviderExecutionReceiptV3,
    outcome_count: u32,
    certificate_route: [u8; 32],
) -> Result<(), CoreSbfError> {
    let resolution_program = account(accounts, RESOLUTION_PROGRAM)?.key;
    let certificate_account = account(accounts, CERTIFICATE)?;
    let bytes =
        read_exact::<RESOLUTION_CERTIFICATE_BYTES_V2>(certificate_account, CoreSbfError::ChildAck)?;
    let certificate =
        ResolutionCertificateV2::decode(&bytes).map_err(|_| CoreSbfError::ChildAck)?;
    authenticate_certificate_route(certificate.route, certificate_route)?;
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
        || !funded_rent_persists_v1(certificate_account.lamports())
        || certificate.kind != ResolutionCertificateKindV2::ResolutionSuccess
        || certificate.market != request.market
        || certificate.source_material != request.source_material
        || certificate.product_record_digest != product_record
        || certificate.provider_evidence != receipt.provider_evidence
        || certificate.funding_allocation != [0; 32]
        || certificate.receipt_account != request.certificate_account
        || certificate.generation != request.generation
        // How many recovery legs this market had entered when it was answered,
        // which is the rung and is zero for a primary capture. Core reads the
        // declared index rather than a literal, so a capture on a rung cannot
        // mint a certificate that reads as the primary's -- and a primary
        // capture still cannot mint one that claims a leg.
        || certificate.attempt_index != u32::from(request.source_index)
        || certificate.schedule_index != 0
        || certificate.selector != receipt.selector
        || certificate.work_paid != 0
        || certificate.funding_remaining != 0
        || certificate.result_numerator != receipt.result_numerator
        || certificate.result_denominator != receipt.result_denominator
        || certificate.observed_at != observed_at
        || certificate
            .validate_terminal_product(request.product_record, outcome_count)
            .is_err()
    {
        return Err(CoreSbfError::ChildAck);
    }
    Ok(())
}

fn validate_outer_frame(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    source_index: u8,
) -> Result<(), CoreSbfError> {
    let expected = if source_index == 0 {
        EXECUTE_PROVIDER_ACCOUNT_COUNT_V3
    } else {
        EXECUTE_PROVIDER_RECOVERY_ACCOUNT_COUNT_V3
    };
    if accounts.len() != expected {
        return Err(CoreSbfError::AccountFrame);
    }
    require_distinct(accounts)?;
    for (index, value) in accounts.iter().enumerate() {
        let signer = index == RESOLVER;
        let writable = matches!(index, SOURCE_STATE | CERTIFICATE | LIFECYCLE);
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

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, CoreSbfError> {
    accounts.get(index).ok_or(CoreSbfError::AccountFrame)
}

#[cfg(test)]
mod provider_route_tests {
    use super::*;
    use dclutch_source::{
        ContentId, RecoveryAttemptV2, SourceAccessProfile, WindowKind, WindowSpecV1,
    };

    struct OwnedAccount {
        key: Pubkey,
        owner: Pubkey,
        lamports: u64,
        data: Vec<u8>,
    }

    impl OwnedAccount {
        fn info(&mut self) -> AccountInfo<'_> {
            AccountInfo::new(
                &self.key,
                false,
                false,
                &mut self.lamports,
                &mut self.data,
                &self.owner,
                false,
            )
        }
    }

    fn id(tag: u8) -> ContentId {
        ContentId::new([tag; 32]).expect("nonzero ID")
    }

    fn record(
        accounts: &mut [OwnedAccount],
        index: usize,
        schema: [u8; 32],
        data: Vec<u8>,
    ) -> [u8; 32] {
        let digest = hash(&data).to_bytes();
        let registry = accounts[REGISTRY].key;
        let (raw, staging, _) = crate::records::derive_record_pdas(&registry, schema, digest);
        accounts[index] = OwnedAccount {
            key: raw,
            owner: registry,
            lamports: 1,
            data,
        };
        accounts[index + 1] = OwnedAccount {
            key: staging,
            owner: system_program::ID,
            lamports: 0,
            data: Vec::new(),
        };
        digest
    }

    fn fixture(recovery: bool) -> (Vec<OwnedAccount>, ProviderExecutionRequestV3, [u8; 32]) {
        let mut accounts: Vec<_> = (0..EXECUTE_PROVIDER_RECOVERY_ACCOUNT_COUNT_V3)
            .map(|_| OwnedAccount {
                key: Pubkey::new_unique(),
                owner: system_program::ID,
                lamports: 1,
                data: Vec::new(),
            })
            .collect();
        let deployment = id(8);
        let provider = ProviderReleaseV1::new(id(1), id(2), deployment, id(3), id(4));
        let route = record(
            &mut accounts,
            21,
            PROVIDER_RELEASE_SCHEMA_ID_V1,
            provider.to_bytes().to_vec(),
        );
        let spec = SourceSpecV1::new(
            id(5),
            id(6),
            ContentId::new(route).expect("route"),
            SourceAccessProfile::PythTerminalOneTransaction,
            id(7),
            id(9),
        );
        let spec_id = record(
            &mut accounts,
            19,
            SOURCE_SPEC_SCHEMA_ID_V1,
            spec.to_bytes().to_vec(),
        );
        let selected = ContentId::new(spec_id).expect("spec");
        let primary = if recovery { id(10) } else { selected };
        let policy = RecoveryPolicyV2::new(
            id(9),
            [
                Some(
                    RecoveryAttemptV2::new(
                        selected,
                        ContentId::new(route).expect("route"),
                        200,
                        id(11),
                    )
                    .expect("attempt"),
                ),
                Some(
                    RecoveryAttemptV2::new(
                        selected,
                        ContentId::new(route).expect("route"),
                        300,
                        id(12),
                    )
                    .expect("attempt"),
                ),
                None,
                None,
            ],
            2,
        )
        .expect("policy");
        let policy_id = ContentId::new(record(
            &mut accounts,
            EXECUTE_PROVIDER_ACCOUNT_COUNT_V3,
            RECOVERY_POLICY_SCHEMA_ID_V2,
            policy.to_bytes().to_vec(),
        ))
        .expect("policy ID");
        let window =
            WindowSpecV1::new(primary, WindowKind::Terminal, 1, 90, 10, 1, id(13)).expect("window");
        let window_id = id(14);
        let material = SourceMaterialV3::explicitly_unbounded(
            id(15),
            primary,
            window_id,
            id(16),
            Some(policy_id),
            id(17),
        );
        let material_id = record(
            &mut accounts,
            17,
            SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
            material.to_bytes().to_vec(),
        );
        let generation = 7_u64;
        let market = accounts[MARKET].key.to_bytes();
        let resolution = accounts[RESOLUTION_PROGRAM].key;
        let (source_key, bump) = Pubkey::find_program_address(
            &[
                dclutch_source::SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
                &market,
                &generation.to_le_bytes(),
            ],
            &resolution,
        );
        let mut source = SourceResolutionStateV2::fresh(
            market,
            generation,
            ContentId::new(material_id).expect("material ID"),
            [18; 32],
            bump,
            0,
            0,
        )
        .expect("source")
        .state();
        if recovery {
            source
                .crank_recovery_ladder(
                    ContentId::new(material_id).expect("material"),
                    material,
                    window_id,
                    window,
                    policy_id,
                    policy,
                    generation,
                    101,
                    0,
                )
                .expect("first rung");
            source
                .crank_recovery_ladder(
                    ContentId::new(material_id).expect("material"),
                    material,
                    window_id,
                    window,
                    policy_id,
                    policy,
                    generation,
                    201,
                    0,
                )
                .expect("second rung");
        }
        accounts[SOURCE_STATE] = OwnedAccount {
            key: source_key,
            owner: resolution,
            lamports: 1,
            data: source.to_bytes().to_vec(),
        };
        let request = ProviderExecutionRequestV3 {
            caller: ProviderCallerV3::Core,
            source_index: if recovery { 2 } else { 0 },
            generation,
            terminal_sequence: source.next_terminal_sequence().expect("sequence"),
            market,
            source_state: source_key.to_bytes(),
            certificate_account: accounts[CERTIFICATE].key.to_bytes(),
            source_material: material_id,
            source_spec: spec_id,
            product_record: id(15).to_bytes(),
            result_domain: [20; 32],
            provider_release: deployment.to_bytes(),
            update_account: [21; 32],
            expected_update_digest: [22; 32],
            provider_submitter: [23; 32],
            resolver: [24; 32],
            caller_program: [25; 32],
            release_set: [26; 32],
            capability_program_set: [0; 32],
            selected_capability_program: [0; 32],
            parent_request_digest: [27; 32],
            post_params_body_digest: [28; 32],
        };
        (accounts, request, route)
    }

    fn authenticate(
        accounts: &mut [OwnedAccount],
        request: &ProviderExecutionRequestV3,
    ) -> Result<[u8; 32], CoreSbfError> {
        let infos: Vec<_> = accounts.iter_mut().map(OwnedAccount::info).collect();
        authenticate_provider_route(&infos, request)
    }

    fn certificate_poststate(
        accounts: &mut [OwnedAccount],
        request: &mut ProviderExecutionRequestV3,
        route: [u8; 32],
    ) -> ProviderExecutionReceiptV3 {
        let key = Pubkey::find_program_address(
            &[
                RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
                &request.source_state,
                &[1],
                &request.terminal_sequence.to_le_bytes(),
            ],
            &accounts[RESOLUTION_PROGRAM].key,
        )
        .0;
        request.certificate_account = key.to_bytes();
        let certificate = ResolutionCertificateV2 {
            kind: ResolutionCertificateKindV2::ResolutionSuccess,
            market: request.market,
            route,
            source_material: request.source_material,
            product_record_digest: request.product_record,
            provider_evidence: [31; 32],
            funding_allocation: [0; 32],
            receipt_account: key.to_bytes(),
            generation: request.generation,
            attempt_index: u32::from(request.source_index),
            schedule_index: 0,
            selector: 0,
            work_paid: 0,
            funding_remaining: 0,
            result_numerator: 100_000_000,
            result_denominator: 1,
            observed_at: 250,
        };
        accounts[CERTIFICATE] = OwnedAccount {
            key,
            owner: accounts[RESOLUTION_PROGRAM].key,
            lamports: 1,
            data: certificate.to_bytes().expect("certificate").to_vec(),
        };
        ProviderExecutionReceiptV3 {
            caller: request.caller,
            generation: request.generation,
            terminal_sequence: request.terminal_sequence,
            request_digest: [32; 32],
            provider_evidence: certificate.provider_evidence,
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
            selector: certificate.selector,
            outcome_count: 4,
            result_numerator: certificate.result_numerator,
            result_denominator: certificate.result_denominator,
            publish_time: 250,
            posted_slot: 100,
            consumed_slot: 101,
        }
    }

    #[test]
    fn terminal_certificate_uses_source_provider_with_distinct_pyth_deployment() {
        for recovery in [false, true] {
            let (mut accounts, mut request, route) = fixture(recovery);
            assert_ne!(route, request.provider_release);
            let selected =
                authenticate(&mut accounts, &request).expect("selected native Source route");
            assert_eq!(selected, route);
            let receipt = certificate_poststate(&mut accounts, &mut request, route);
            let infos: Vec<_> = accounts.iter_mut().map(OwnedAccount::info).collect();
            assert_eq!(
                authenticate_certificate(
                    &infos,
                    request.product_record,
                    &request,
                    &receipt,
                    4,
                    selected
                ),
                Ok(())
            );
            assert_eq!(authenticate_certificate_route(route, selected), Ok(()));
            assert_eq!(
                authenticate_certificate_route(request.provider_release, selected),
                Err(CoreSbfError::ChildAck)
            );
            assert_eq!(
                authenticate_certificate_route([99; 32], selected),
                Err(CoreSbfError::ChildAck)
            );
        }
    }

    #[test]
    fn selected_provider_refuses_valid_record_substitutions_and_wrong_active_rung() {
        for recovery in [false, true] {
            let (mut accounts, mut request, _) = fixture(recovery);
            assert!(authenticate(&mut accounts, &request).is_ok());
            request.provider_release = [99; 32];
            assert_eq!(
                authenticate(&mut accounts, &request),
                Err(CoreSbfError::Reference)
            );
            let (mut accounts, mut request, _) = fixture(recovery);
            let mut spec = SourceSpecV1::decode(&accounts[19].data).expect("spec");
            spec = SourceSpecV1::new(
                id(99),
                spec.unit_id(),
                spec.provider_release_id(),
                spec.access_profile(),
                spec.adapter_config_id(),
                spec.capacity_profile_id(),
            );
            request.source_spec = record(
                &mut accounts,
                19,
                SOURCE_SPEC_SCHEMA_ID_V1,
                spec.to_bytes().to_vec(),
            );
            assert_eq!(
                authenticate(&mut accounts, &request),
                Err(CoreSbfError::Reference)
            );
            let (mut accounts, request, _) = fixture(recovery);
            accounts[21].owner = Pubkey::new_unique();
            assert_eq!(
                authenticate(&mut accounts, &request),
                Err(CoreSbfError::FinalizedRecord)
            );
            let (mut accounts, request, _) = fixture(recovery);
            accounts[22].data.push(1);
            assert_eq!(
                authenticate(&mut accounts, &request),
                Err(CoreSbfError::FinalizedRecord)
            );
        }
        let (mut accounts, mut request, _) = fixture(true);
        request.source_index = 1;
        assert_eq!(
            authenticate(&mut accounts, &request),
            Err(CoreSbfError::Reference)
        );
    }
}
