//! Canonical Market-Core effect route for Source creation, readiness, terminal admission, and close.

use alloc::boxed::Box;
use core::convert::TryFrom;

use dclutch_capability_contract::{
    CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityFundingLedgerDerivationV2,
    CapabilityManifestV1, ContentId as CapabilityContentId, FUNDING_LEDGER_HEADER_BYTES_V2,
    FUNDING_LEDGER_SLOT_BYTES_V2, FundingLedgerCloseCustodyV2, FundingLedgerStatusV2,
    FundingLedgerV2, funding_ledger_bytes_v2,
};
use dclutch_market_core_codec::{
    CAPABILITY_FUNDING_HEADER_BYTES_V2, CORE_EFFECT_ACK_BYTES_V1, CORE_EFFECT_DIGEST_DOMAIN_V1,
    CORE_EFFECT_ENVELOPE_BYTES_V1, CapabilityFundingHeaderV2, CoreEffectAckV1, CoreEffectActionV1,
    CoreEffectEnvelopeV1, CoreState, Identity, MarketCoreStateSeedsV2, Phase as CorePhase,
    Readiness as CoreReadiness, Role,
};
use dclutch_product_runtime_v2::ContentId as ProductContentId;
use dclutch_product_runtime_v2_svm_reader::{
    AuthenticatedProductRuntimeV2, FinalizedRecordFrameV2, ProductRuntimeFrameV2,
    authenticate_product_runtime_v2,
};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetViewV1,
};
use dclutch_release_set_contract::ExecutionRoleV1;
use dclutch_resolution_codec::{
    DIRECT_FUNDING_CLOSE_REQUEST_BYTES_V1, DIRECT_FUNDING_CLOSE_REQUEST_MAGIC_V1,
    DirectFundingCloseRequestV1, FUNDING_ACTIVATION_RECEIPT_BYTES_V1,
    FUNDING_ACTIVATION_RECEIPT_PDA_DOMAIN_V1, FUNDING_ACTIVATION_REQUEST_BYTES_V1,
    FUNDING_ACTIVATION_REQUEST_MAGIC_V1, FundingActivationReceiptV1, FundingActivationRequestV1,
    RESOLUTION_CERTIFICATE_BYTES_V2, RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
    RESOLUTION_CONTROLLER_RELEASE_ID_V7, RESOLUTION_CORE_ROLE_REQUEST_BYTES_V2,
    RESOLUTION_POSTSTATE_DIGEST_DOMAIN_V2, ResolutionCertificateKindV2, ResolutionCertificateV2,
    ResolutionCoreActionV1, ResolutionCoreReceiptKindV1, ResolutionRoleRequestV2,
    SOURCE_CLOSURE_RECEIPT_BYTES_V3, SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V3,
    SOURCE_FUNDING_SET_DIGEST_DOMAIN_V2, SourceClosureReceiptV3,
    funding_lifecycle_account_digest_v1,
};
use dclutch_source_contract::{
    RECOVERY_POLICY_SCHEMA_ID_V2, RecoveryPolicyV2, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
    SOURCE_RESOLUTION_STATE_BYTES_V2, SourceMaterialV3, SourceResolutionPhaseV1,
    SourceResolutionRouteV1, SourceResolutionStateV2,
};
use solana_program::{
    account_info::{AccountInfo, next_account_info},
    entrypoint::ProgramResult,
    hash::{hash, hashv},
    program::{invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
};
use solana_sdk_ids::system_program;
use solana_system_interface::instruction::{allocate, assign};

use crate::{
    RecordKind, ResolutionError, authenticate_clock, authenticate_finalized_record,
    authenticate_rent, deployment_observation,
};

/// Exact fixed instruction width for one canonical Core envelope and Resolution request.
pub(crate) const CORE_EFFECT_INSTRUCTION_BYTES: usize = CORE_EFFECT_ENVELOPE_BYTES_V1
    + CAPABILITY_FUNDING_HEADER_BYTES_V2
    + RESOLUTION_CORE_ROLE_REQUEST_BYTES_V2;
/// Create: common fourteen, Rent, System, and finalized RecoveryPolicyV2 raw/staging.
pub(crate) const CREATE_FUND_ACCOUNT_COUNT: usize = 18;
/// Verify: common fourteen, beneficiary, Clock, Rent, and finalized recovery raw/staging.
pub(crate) const VERIFY_FUND_ACCOUNT_COUNT: usize = 19;
/// Terminal admission: common fourteen, certificate, Rent, and six Product graph records.
pub(crate) const ADMIT_TERMINAL_ACCOUNT_COUNT: usize = 22;
/// Close: common fourteen, certificate/closure/beneficiary/Clock/Rent/System, and recovery pair.
pub(crate) const CLOSE_FUND_ACCOUNT_COUNT: usize = 22;
/// Direct activation: fixed eighteen accounts and optional finalized RecoveryPolicy pair.
pub(crate) const DIRECT_FUNDING_ACTIVATION_ACCOUNT_COUNT_V1: usize = 20;
/// Direct close: fixed nineteen accounts and optional finalized RecoveryPolicy pair.
pub(crate) const DIRECT_FUNDING_CLOSE_ACCOUNT_COUNT_V1: usize = 21;

const RESOLUTION_FUNDING_LEDGER_BYTES: usize =
    FUNDING_LEDGER_HEADER_BYTES_V2 + 3 * FUNDING_LEDGER_SLOT_BYTES_V2;

#[derive(Clone, Copy)]
struct CommonAccounts<'a, 'info> {
    caller_authority: &'a AccountInfo<'info>,
    market: &'a AccountInfo<'info>,
    activated_release_set: &'a AccountInfo<'info>,
    registry_program: &'a AccountInfo<'info>,
    core_program: &'a AccountInfo<'info>,
    core_programdata: &'a AccountInfo<'info>,
    resolution_program: &'a AccountInfo<'info>,
    resolution_programdata: &'a AccountInfo<'info>,
    source_material: &'a AccountInfo<'info>,
    source_material_staging: &'a AccountInfo<'info>,
    capability_manifest: &'a AccountInfo<'info>,
    capability_manifest_staging: &'a AccountInfo<'info>,
    source_state: &'a AccountInfo<'info>,
    funding_ledger: &'a AccountInfo<'info>,
}

struct AuthenticatedCore {
    state: CoreState,
    full_effect_digest: Identity,
}

#[derive(Clone, Copy)]
struct DirectFundingAccounts<'a, 'info> {
    market: &'a AccountInfo<'info>,
    activated_release_set: &'a AccountInfo<'info>,
    registry_program: &'a AccountInfo<'info>,
    core_program: &'a AccountInfo<'info>,
    core_programdata: &'a AccountInfo<'info>,
    resolution_program: &'a AccountInfo<'info>,
    resolution_programdata: &'a AccountInfo<'info>,
    source_material: &'a AccountInfo<'info>,
    source_material_staging: &'a AccountInfo<'info>,
    capability_manifest: &'a AccountInfo<'info>,
    capability_manifest_staging: &'a AccountInfo<'info>,
    source_state: &'a AccountInfo<'info>,
    funding_ledger: &'a AccountInfo<'info>,
    beneficiary: &'a AccountInfo<'info>,
    receipt: &'a AccountInfo<'info>,
    clock: &'a AccountInfo<'info>,
    rent: &'a AccountInfo<'info>,
    system: &'a AccountInfo<'info>,
}

#[derive(Clone, Copy)]
struct DirectCloseAccounts<'a, 'info> {
    market: &'a AccountInfo<'info>,
    activated_release_set: &'a AccountInfo<'info>,
    registry_program: &'a AccountInfo<'info>,
    core_program: &'a AccountInfo<'info>,
    core_programdata: &'a AccountInfo<'info>,
    resolution_program: &'a AccountInfo<'info>,
    resolution_programdata: &'a AccountInfo<'info>,
    source_material: &'a AccountInfo<'info>,
    source_material_staging: &'a AccountInfo<'info>,
    capability_manifest: &'a AccountInfo<'info>,
    capability_manifest_staging: &'a AccountInfo<'info>,
    source_state: &'a AccountInfo<'info>,
    funding_ledger: &'a AccountInfo<'info>,
    certificate: &'a AccountInfo<'info>,
    closure: &'a AccountInfo<'info>,
    beneficiary: &'a AccountInfo<'info>,
    clock: &'a AccountInfo<'info>,
    rent: &'a AccountInfo<'info>,
    system: &'a AccountInfo<'info>,
}

/// Return whether bytes select the one canonical Core effect route.
pub(crate) fn is_core_effect(instruction_data: &[u8]) -> bool {
    instruction_data.len() == CORE_EFFECT_INSTRUCTION_BYTES
        && instruction_data.get(..8)
            == Some(dclutch_market_core_codec::CORE_EFFECT_MAGIC_V1.as_slice())
}

/// Return whether bytes select the V7 permissionless activation route.
pub(crate) fn is_direct_funding_activation_v1(instruction_data: &[u8]) -> bool {
    instruction_data.len() == FUNDING_ACTIVATION_REQUEST_BYTES_V1
        && instruction_data.get(..8) == Some(FUNDING_ACTIVATION_REQUEST_MAGIC_V1.as_slice())
}

/// Return whether bytes select the V7 permissionless terminal-close route.
pub(crate) fn is_direct_funding_close_v1(instruction_data: &[u8]) -> bool {
    instruction_data.len() == DIRECT_FUNDING_CLOSE_REQUEST_BYTES_V1
        && instruction_data.get(..8) == Some(DIRECT_FUNDING_CLOSE_REQUEST_MAGIC_V1.as_slice())
}

/// Activate one exact Pending ledger and persist the immutable receipt last.
#[inline(never)]
pub(crate) fn process_direct_funding_activation_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let request = Box::new(
        FundingActivationRequestV1::decode(instruction_data)
            .map_err(|_| ResolutionError::Instruction)?,
    );
    let direct = parse_direct_funding_accounts(program_id, accounts, request.as_ref())?;
    let rent = authenticate_rent(direct.rent)?;
    let clock = authenticate_clock(direct.clock)?;
    if clock.slot == 0 {
        return Err(ResolutionError::Sysvar.into());
    }
    let state = authenticate_direct_market(direct, request.as_ref())?;
    authenticate_direct_activation(program_id, direct, request.as_ref())?;
    authenticate_direct_source_records(direct, request.as_ref(), &rent)?;
    let material_data = direct
        .source_material
        .try_borrow_data()
        .map_err(|_| ResolutionError::SourceMaterial)?;
    let material =
        SourceMaterialV3::decode(&material_data).map_err(|_| ResolutionError::SourceMaterial)?;
    let manifest_data = direct
        .capability_manifest
        .try_borrow_data()
        .map_err(|_| ResolutionError::Funding)?;
    let manifest =
        CapabilityManifestV1::decode(&manifest_data).map_err(|_| ResolutionError::Funding)?;
    let recovery_policy = authenticate_direct_recovery_policy(
        direct,
        accounts.get(18),
        accounts.get(19),
        material,
        &rent,
    )?;
    authenticate_funding_entries(material, recovery_policy, manifest, request.role)?;
    authenticate_direct_source(program_id, direct, request.as_ref(), state)?;
    let manifest_id = CapabilityContentId::new(request.role.capability_manifest)
        .map_err(|_| ResolutionError::Funding)?;
    let request_digest = request.digest().map_err(|_| ResolutionError::Instruction)?;

    commit_direct_activation(
        program_id,
        direct,
        request.as_ref(),
        request_digest,
        state.identity.generation,
        manifest_id,
        manifest,
        clock.slot,
        &rent,
    )
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn commit_direct_activation(
    program_id: &Pubkey,
    direct: DirectFundingAccounts<'_, '_>,
    request: &FundingActivationRequestV1,
    request_digest: [u8; 32],
    generation: u64,
    manifest_id: CapabilityContentId,
    manifest: CapabilityManifestV1<'_>,
    activation_slot: u64,
    rent: &Rent,
) -> ProgramResult {
    if direct.receipt.owner == program_id {
        return authenticate_completed_activation(
            program_id,
            direct,
            *request,
            request_digest,
            manifest_id,
            manifest,
            rent,
        );
    }

    require_prepaid_output(
        direct.receipt,
        rent.minimum_balance(FUNDING_ACTIVATION_RECEIPT_BYTES_V1),
    )?;
    let mut ledger_bytes = Box::new(copy_ledger_bytes(direct.funding_ledger)?);
    let pending_digest = funding_lifecycle_account_digest_v1(
        direct.funding_ledger.owner.to_bytes(),
        direct.funding_ledger.key.to_bytes(),
        direct.funding_ledger.lamports(),
        ledger_bytes.as_ref(),
    );
    if pending_digest != request.expected_pending_ledger_digest {
        return Err(ResolutionError::Funding.into());
    }
    authenticate_direct_ledger(
        program_id,
        direct,
        generation,
        manifest_id,
        manifest,
        request.role,
        FundingLedgerStatusV2::Pending,
        ledger_bytes.as_ref(),
        direct.funding_ledger.lamports(),
        &rent,
        false,
    )?;
    let mut beneficiary_credit = 0_u64;
    for entry_index in [
        request.role.recovery_entry_index,
        request.role.exhaustion_entry_index,
        request.role.failure_entry_index,
    ] {
        let debit = FundingLedgerV2::activate_in_place(
            ledger_bytes.as_mut(),
            manifest_id,
            manifest,
            entry_index,
            activation_slot,
        )
        .map_err(|_| ResolutionError::Funding)?;
        beneficiary_credit = beneficiary_credit
            .checked_add(debit.rent_lamports())
            .and_then(|value| value.checked_add(debit.creation_lamports()))
            .ok_or(ResolutionError::Arithmetic)?;
    }
    let post_ledger_lamports = direct
        .funding_ledger
        .lamports()
        .checked_sub(beneficiary_credit)
        .ok_or(ResolutionError::Arithmetic)?;
    let beneficiary_lamports = direct
        .beneficiary
        .lamports()
        .checked_add(beneficiary_credit)
        .ok_or(ResolutionError::Arithmetic)?;
    authenticate_direct_ledger(
        program_id,
        direct,
        generation,
        manifest_id,
        manifest,
        request.role,
        FundingLedgerStatusV2::Active,
        ledger_bytes.as_ref(),
        post_ledger_lamports,
        &rent,
        false,
    )?;
    let active = FundingLedgerV2::decode(ledger_bytes.as_ref())
        .and_then(|ledger| ledger.authenticate(manifest_id, manifest))
        .map_err(|_| ResolutionError::Funding)?;
    let ledger_rent_lamports = rent.minimum_balance(RESOLUTION_FUNDING_LEDGER_BYTES);
    let remaining_native_principal_lamports = active
        .remaining_native_lamports_total()
        .map_err(|_| ResolutionError::Funding)?;
    let active_ledger_digest = funding_lifecycle_account_digest_v1(
        program_id.to_bytes(),
        direct.funding_ledger.key.to_bytes(),
        post_ledger_lamports,
        ledger_bytes.as_ref(),
    );
    let receipt = FundingActivationReceiptV1 {
        request_digest,
        release_set: request.release_set,
        resolution_release: RESOLUTION_CONTROLLER_RELEASE_ID_V7,
        market: request.market,
        generation: request.generation,
        role: request.role,
        market_state_digest: request.expected_market_state_digest,
        source_state_digest: request.expected_source_state_digest,
        pending_ledger_digest: pending_digest,
        active_ledger_digest,
        activation_slot,
        beneficiary_credit_lamports: beneficiary_credit,
        ledger_rent_lamports,
        remaining_native_principal_lamports,
        post_ledger_lamports,
        producer: program_id.to_bytes(),
    };
    let receipt_bytes = Box::new(receipt.encode().map_err(|_| ResolutionError::OutputState)?);
    commit_activated_ledger(
        direct.funding_ledger,
        ledger_bytes.as_ref(),
        post_ledger_lamports,
        direct.beneficiary,
        beneficiary_lamports,
    )?;
    initialize_activation_receipt(
        program_id,
        direct.market,
        direct.receipt,
        request.generation,
        direct.system,
        &rent,
    )?;
    write_state(direct.receipt, receipt_bytes.as_ref())?;
    set_return_data(receipt_bytes.as_ref());
    Ok(())
}

/// Close one exact Retiring/Consumed terminal Source and ledger without a Core CPI.
#[inline(never)]
pub(crate) fn process_direct_funding_close_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let request = Box::new(
        DirectFundingCloseRequestV1::decode(instruction_data)
            .map_err(|_| ResolutionError::Instruction)?,
    );
    let direct = parse_direct_close_accounts(program_id, accounts, request.as_ref())?;
    let rent = authenticate_rent(direct.rent)?;
    let clock = authenticate_clock(direct.clock)?;
    if clock.unix_timestamp <= 0 {
        return Err(ResolutionError::Sysvar.into());
    }
    let state = authenticate_direct_close_market(direct, request.as_ref())?;
    authenticate_direct_close_release(program_id, direct, request.as_ref())?;
    authenticate_direct_close_records(direct, request.as_ref(), &rent)?;
    let material_data = direct
        .source_material
        .try_borrow_data()
        .map_err(|_| ResolutionError::SourceMaterial)?;
    let material =
        SourceMaterialV3::decode(&material_data).map_err(|_| ResolutionError::SourceMaterial)?;
    let recovery_policy = authenticate_direct_close_recovery_policy(
        direct,
        accounts.get(19),
        accounts.get(20),
        material,
        &rent,
    )?;
    let manifest_data = direct
        .capability_manifest
        .try_borrow_data()
        .map_err(|_| ResolutionError::Funding)?;
    let manifest =
        CapabilityManifestV1::decode(&manifest_data).map_err(|_| ResolutionError::Funding)?;
    authenticate_funding_entries(material, recovery_policy, manifest, request.role)?;
    let manifest_id = CapabilityContentId::new(request.role.capability_manifest)
        .map_err(|_| ResolutionError::Funding)?;

    commit_direct_close(
        program_id,
        direct,
        request.as_ref(),
        state,
        manifest_id,
        manifest,
        clock.unix_timestamp,
        &rent,
    )
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn commit_direct_close(
    program_id: &Pubkey,
    direct: DirectCloseAccounts<'_, '_>,
    request: &DirectFundingCloseRequestV1,
    state: CoreState,
    manifest_id: CapabilityContentId,
    manifest: CapabilityManifestV1<'_>,
    close_time: i64,
    rent: &Rent,
) -> ProgramResult {
    let source_data = direct
        .source_state
        .try_borrow_data()
        .map_err(|_| ResolutionError::OutputState)?;
    if hash(&source_data).to_bytes() != request.source_state_digest {
        return Err(ResolutionError::Transition.into());
    }
    let mut source =
        SourceResolutionStateV2::decode(&source_data).map_err(|_| ResolutionError::OutputState)?;
    authenticate_state_account_v2(program_id, direct.source_state, source)?;
    if !matches!(
        source.phase(),
        SourceResolutionPhaseV1::Resolved | SourceResolutionPhaseV1::FailureCommitted
    ) || source.market() != request.market
        || source.generation() != request.generation
        || source.material_id().to_bytes() != request.role.source_material
        || source.rent_beneficiary() != request.role.beneficiary
    {
        return Err(ResolutionError::Transition.into());
    }
    let terminal = source
        .terminal_projection()
        .map_err(|_| ResolutionError::Transition)?;
    if terminal.selector() != state.terminal_winner
        || terminal
            .terminal_sequence()
            .checked_add(1)
            .ok_or(ResolutionError::Arithmetic)?
            != request.role.receipt_sequence
    {
        return Err(ResolutionError::Transition.into());
    }
    source
        .retire(state.identity.generation, close_time, 1, 1)
        .map_err(|_| ResolutionError::Transition)?;

    let mut closed_ledger = Box::new(copy_ledger_bytes(direct.funding_ledger)?);
    let ledger_account_digest = funding_lifecycle_account_digest_v1(
        direct.funding_ledger.owner.to_bytes(),
        direct.funding_ledger.key.to_bytes(),
        direct.funding_ledger.lamports(),
        closed_ledger.as_ref(),
    );
    if ledger_account_digest != request.funding_ledger_digest {
        return Err(ResolutionError::Funding.into());
    }
    authenticate_direct_close_ledger(
        program_id,
        direct,
        state.identity.generation,
        manifest_id,
        manifest,
        request.role,
        FundingLedgerStatusV2::Active,
        closed_ledger.as_ref(),
        direct.funding_ledger.lamports(),
        rent,
        true,
    )?;
    let funding_set_digest = funding_set_digest(closed_ledger.as_ref());
    let mut ledger_can_close = false;
    let mut planned_ledger_lamports = direct.funding_ledger.lamports();
    let mut ledger_remaining_native_principal = 0_u64;
    let mut ledger_rent_lamports = 0_u64;
    let mut ledger_lamport_surplus = 0_u64;
    for entry_index in [
        request.role.recovery_entry_index,
        request.role.exhaustion_entry_index,
        request.role.failure_entry_index,
    ] {
        let plan = FundingLedgerV2::close_slot_in_place(
            closed_ledger.as_mut(),
            manifest_id,
            manifest,
            entry_index,
            FundingLedgerCloseCustodyV2::native_only(
                planned_ledger_lamports,
                rent.minimum_balance(RESOLUTION_FUNDING_LEDGER_BYTES),
                request.role.beneficiary,
            )
            .map_err(|_| ResolutionError::Funding)?,
        )
        .map_err(|_| ResolutionError::Funding)?;
        if plan.native_rent_credit() != request.role.beneficiary
            || plan.remaining_realm_collateral() != 0
            || plan.realm_token_beneficiary().is_some()
        {
            return Err(ResolutionError::Funding.into());
        }
        ledger_remaining_native_principal = ledger_remaining_native_principal
            .checked_add(plan.remaining_native_lamports())
            .ok_or(ResolutionError::Arithmetic)?;
        if plan.ledger_can_close() {
            ledger_rent_lamports = plan.ledger_rent_lamports();
            ledger_lamport_surplus = plan.ledger_lamport_donation();
        } else if plan.ledger_rent_lamports() != 0 || plan.ledger_lamport_donation() != 0 {
            return Err(ResolutionError::Funding.into());
        }
        planned_ledger_lamports = plan.expected_post_ledger_lamports();
        ledger_can_close = plan.ledger_can_close();
    }
    if !ledger_can_close || planned_ledger_lamports != 0 {
        return Err(ResolutionError::Funding.into());
    }

    let certificate_data = direct
        .certificate
        .try_borrow_data()
        .map_err(|_| ResolutionError::OutputState)?;
    if hash(&certificate_data).to_bytes() != request.certificate_digest {
        return Err(ResolutionError::OutputState.into());
    }
    let terminal_kind = if terminal.route() == SourceResolutionRouteV1::Failure {
        ResolutionCoreReceiptKindV1::TerminalFailure
    } else {
        ResolutionCoreReceiptKindV1::TerminalSuccess
    };
    authenticate_admitted_terminal_certificate_v2(
        program_id,
        direct.source_state,
        direct.certificate,
        terminal_kind,
        terminal.terminal_sequence(),
        request.role.source_material,
        request.market,
        state.identity.product_record.to_bytes(),
        request.generation,
        terminal.selector(),
        &certificate_data,
        rent,
    )?;
    let ledger_refund = direct.funding_ledger.lamports();
    let source_refund = direct.source_state.lamports();
    if source_refund < rent.minimum_balance(SOURCE_RESOLUTION_STATE_BYTES_V2)
        || ledger_remaining_native_principal
            .checked_add(ledger_rent_lamports)
            .and_then(|value| value.checked_add(ledger_lamport_surplus))
            != Some(ledger_refund)
    {
        return Err(ResolutionError::Funding.into());
    }
    let refund_lamports = source_refund
        .checked_add(ledger_remaining_native_principal)
        .and_then(|value| value.checked_add(ledger_rent_lamports))
        .and_then(|value| value.checked_add(ledger_lamport_surplus))
        .ok_or(ResolutionError::Arithmetic)?;
    let beneficiary_lamports = direct
        .beneficiary
        .lamports()
        .checked_add(refund_lamports)
        .ok_or(ResolutionError::Arithmetic)?;
    let closure = SourceClosureReceiptV3 {
        market: request.market,
        source_state: direct.source_state.key.to_bytes(),
        source_material: request.role.source_material,
        capability_manifest: request.role.capability_manifest,
        terminal_certificate: direct.certificate.key.to_bytes(),
        receipt_account: direct.closure.key.to_bytes(),
        beneficiary: request.role.beneficiary,
        source_state_digest: request.source_state_digest,
        terminal_certificate_digest: request.certificate_digest,
        funding_set_digest,
        generation: request.generation,
        terminal_sequence: terminal.terminal_sequence(),
        selector: terminal.selector(),
        source_refund_lamports: source_refund,
        ledger_remaining_native_principal,
        ledger_rent_lamports,
        ledger_lamport_surplus,
        refund_lamports,
        closed_at: u64::try_from(close_time).map_err(|_| ResolutionError::Arithmetic)?,
    };
    let closure_bytes = Box::new(
        closure
            .to_bytes()
            .map_err(|_| ResolutionError::OutputState)?,
    );
    drop(certificate_data);
    drop(source_data);
    initialize_closure_output(
        program_id,
        direct.source_state,
        direct.closure,
        request.role.receipt_sequence,
        direct.system,
        rent,
    )?;
    write_state(direct.closure, closure_bytes.as_ref())?;
    commit_refund(
        direct.source_state,
        direct.funding_ledger,
        direct.beneficiary,
        beneficiary_lamports,
    )?;
    set_return_data(closure_bytes.as_ref());
    Ok(())
}

fn parse_direct_funding_accounts<'a, 'info>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'info>],
    request: &FundingActivationRequestV1,
) -> Result<DirectFundingAccounts<'a, 'info>, ProgramError> {
    if accounts.len() != DIRECT_FUNDING_ACTIVATION_ACCOUNT_COUNT_V1
        && accounts.len() != DIRECT_FUNDING_ACTIVATION_ACCOUNT_COUNT_V1.saturating_sub(2)
    {
        return Err(ResolutionError::AccountFrame.into());
    }
    for (index, account) in accounts.iter().enumerate() {
        if account.is_signer
            || accounts
                .iter()
                .skip(index.checked_add(1).ok_or(ResolutionError::Arithmetic)?)
                .any(|other| other.key == account.key)
        {
            return Err(ResolutionError::AccountFrame.into());
        }
    }
    let direct = DirectFundingAccounts {
        market: accounts.first().ok_or(ResolutionError::AccountFrame)?,
        activated_release_set: accounts.get(1).ok_or(ResolutionError::AccountFrame)?,
        registry_program: accounts.get(2).ok_or(ResolutionError::AccountFrame)?,
        core_program: accounts.get(3).ok_or(ResolutionError::AccountFrame)?,
        core_programdata: accounts.get(4).ok_or(ResolutionError::AccountFrame)?,
        resolution_program: accounts.get(5).ok_or(ResolutionError::AccountFrame)?,
        resolution_programdata: accounts.get(6).ok_or(ResolutionError::AccountFrame)?,
        source_material: accounts.get(7).ok_or(ResolutionError::AccountFrame)?,
        source_material_staging: accounts.get(8).ok_or(ResolutionError::AccountFrame)?,
        capability_manifest: accounts.get(9).ok_or(ResolutionError::AccountFrame)?,
        capability_manifest_staging: accounts.get(10).ok_or(ResolutionError::AccountFrame)?,
        source_state: accounts.get(11).ok_or(ResolutionError::AccountFrame)?,
        funding_ledger: accounts.get(12).ok_or(ResolutionError::AccountFrame)?,
        beneficiary: accounts.get(13).ok_or(ResolutionError::AccountFrame)?,
        receipt: accounts.get(14).ok_or(ResolutionError::AccountFrame)?,
        clock: accounts.get(15).ok_or(ResolutionError::AccountFrame)?,
        rent: accounts.get(16).ok_or(ResolutionError::AccountFrame)?,
        system: accounts.get(17).ok_or(ResolutionError::AccountFrame)?,
    };
    if direct.market.is_writable
        || direct.market.executable
        || direct.activated_release_set.is_writable
        || direct.activated_release_set.executable
        || !direct.registry_program.executable
        || direct.registry_program.is_writable
        || !direct.core_program.executable
        || direct.core_program.is_writable
        || direct.core_programdata.is_writable
        || direct.core_programdata.executable
        || direct.resolution_program.key != program_id
        || !direct.resolution_program.executable
        || direct.resolution_program.is_writable
        || direct.resolution_programdata.is_writable
        || direct.resolution_programdata.executable
        || direct.source_material.is_writable
        || direct.source_material.executable
        || direct.source_material_staging.is_writable
        || direct.source_material_staging.executable
        || direct.capability_manifest.is_writable
        || direct.capability_manifest.executable
        || direct.capability_manifest_staging.is_writable
        || direct.capability_manifest_staging.executable
        || direct.source_state.is_writable
        || direct.source_state.executable
        || !direct.funding_ledger.is_writable
        || direct.funding_ledger.executable
        || !direct.beneficiary.is_writable
        || direct.beneficiary.executable
        || !direct.receipt.is_writable
        || direct.receipt.executable
        || direct.clock.is_writable
        || direct.clock.executable
        || direct.rent.is_writable
        || direct.rent.executable
        || direct.system.key != &system_program::ID
        || !direct.system.executable
        || direct.system.is_writable
        || direct.market.key.to_bytes() != request.market
        || direct.source_state.key.to_bytes() != request.role.source_state
        || direct.funding_ledger.key.to_bytes() != request.role.funding_ledger
        || direct.beneficiary.key.to_bytes() != request.role.beneficiary
        || direct.receipt.key.to_bytes() != request.receipt
    {
        return Err(ResolutionError::AccountFrame.into());
    }
    for account in accounts.iter().skip(18) {
        if account.is_writable || account.executable {
            return Err(ResolutionError::AccountFrame.into());
        }
    }
    Ok(direct)
}

fn parse_direct_close_accounts<'a, 'info>(
    program_id: &Pubkey,
    accounts: &'a [AccountInfo<'info>],
    request: &DirectFundingCloseRequestV1,
) -> Result<DirectCloseAccounts<'a, 'info>, ProgramError> {
    if accounts.len() != DIRECT_FUNDING_CLOSE_ACCOUNT_COUNT_V1
        && accounts.len() != DIRECT_FUNDING_CLOSE_ACCOUNT_COUNT_V1.saturating_sub(2)
    {
        return Err(ResolutionError::AccountFrame.into());
    }
    for (index, account) in accounts.iter().enumerate() {
        if account.is_signer
            || accounts
                .iter()
                .skip(index.checked_add(1).ok_or(ResolutionError::Arithmetic)?)
                .any(|other| other.key == account.key)
        {
            return Err(ResolutionError::AccountFrame.into());
        }
    }
    let direct = DirectCloseAccounts {
        market: accounts.first().ok_or(ResolutionError::AccountFrame)?,
        activated_release_set: accounts.get(1).ok_or(ResolutionError::AccountFrame)?,
        registry_program: accounts.get(2).ok_or(ResolutionError::AccountFrame)?,
        core_program: accounts.get(3).ok_or(ResolutionError::AccountFrame)?,
        core_programdata: accounts.get(4).ok_or(ResolutionError::AccountFrame)?,
        resolution_program: accounts.get(5).ok_or(ResolutionError::AccountFrame)?,
        resolution_programdata: accounts.get(6).ok_or(ResolutionError::AccountFrame)?,
        source_material: accounts.get(7).ok_or(ResolutionError::AccountFrame)?,
        source_material_staging: accounts.get(8).ok_or(ResolutionError::AccountFrame)?,
        capability_manifest: accounts.get(9).ok_or(ResolutionError::AccountFrame)?,
        capability_manifest_staging: accounts.get(10).ok_or(ResolutionError::AccountFrame)?,
        source_state: accounts.get(11).ok_or(ResolutionError::AccountFrame)?,
        funding_ledger: accounts.get(12).ok_or(ResolutionError::AccountFrame)?,
        certificate: accounts.get(13).ok_or(ResolutionError::AccountFrame)?,
        closure: accounts.get(14).ok_or(ResolutionError::AccountFrame)?,
        beneficiary: accounts.get(15).ok_or(ResolutionError::AccountFrame)?,
        clock: accounts.get(16).ok_or(ResolutionError::AccountFrame)?,
        rent: accounts.get(17).ok_or(ResolutionError::AccountFrame)?,
        system: accounts.get(18).ok_or(ResolutionError::AccountFrame)?,
    };
    if direct.market.is_writable
        || direct.market.executable
        || direct.activated_release_set.is_writable
        || direct.activated_release_set.executable
        || !direct.registry_program.executable
        || direct.registry_program.is_writable
        || !direct.core_program.executable
        || direct.core_program.is_writable
        || direct.core_programdata.is_writable
        || direct.core_programdata.executable
        || direct.resolution_program.key != program_id
        || !direct.resolution_program.executable
        || direct.resolution_program.is_writable
        || direct.resolution_programdata.is_writable
        || direct.resolution_programdata.executable
        || direct.source_material.is_writable
        || direct.source_material.executable
        || direct.source_material_staging.is_writable
        || direct.source_material_staging.executable
        || direct.capability_manifest.is_writable
        || direct.capability_manifest.executable
        || direct.capability_manifest_staging.is_writable
        || direct.capability_manifest_staging.executable
        || !direct.source_state.is_writable
        || direct.source_state.executable
        || !direct.funding_ledger.is_writable
        || direct.funding_ledger.executable
        || direct.certificate.is_writable
        || direct.certificate.executable
        || !direct.closure.is_writable
        || direct.closure.executable
        || !direct.beneficiary.is_writable
        || direct.beneficiary.executable
        || direct.clock.is_writable
        || direct.clock.executable
        || direct.rent.is_writable
        || direct.rent.executable
        || direct.system.key != &system_program::ID
        || !direct.system.executable
        || direct.system.is_writable
        || direct.market.key.to_bytes() != request.market
        || direct.source_state.key.to_bytes() != request.role.source_state
        || direct.funding_ledger.key.to_bytes() != request.role.funding_ledger
        || direct.closure.key.to_bytes() != request.role.receipt
        || direct.beneficiary.key.to_bytes() != request.role.beneficiary
    {
        return Err(ResolutionError::AccountFrame.into());
    }
    for account in accounts.iter().skip(19) {
        if account.is_writable || account.executable {
            return Err(ResolutionError::AccountFrame.into());
        }
    }
    let closure_data = direct
        .closure
        .try_borrow_data()
        .map_err(|_| ResolutionError::OutputState)?;
    if direct.closure.owner != &system_program::ID
        || !closure_data.is_empty()
        || funding_lifecycle_account_digest_v1(
            direct.closure.owner.to_bytes(),
            direct.closure.key.to_bytes(),
            direct.closure.lamports(),
            &closure_data,
        ) != request.closure_prestate_digest
    {
        return Err(ResolutionError::OutputState.into());
    }
    Ok(direct)
}

fn authenticate_direct_close_market(
    direct: DirectCloseAccounts<'_, '_>,
    request: &DirectFundingCloseRequestV1,
) -> Result<CoreState, ProgramError> {
    if direct.market.owner != direct.core_program.key {
        return Err(ResolutionError::MarketAuthority.into());
    }
    let market_data = direct
        .market
        .try_borrow_data()
        .map_err(|_| ResolutionError::MarketAuthority)?;
    if hash(&market_data).to_bytes() != request.market_state_digest {
        return Err(ResolutionError::MarketAuthority.into());
    }
    let state = CoreState::decode(&market_data).map_err(|_| ResolutionError::MarketAuthority)?;
    let seeds = MarketCoreStateSeedsV2::new(state.identity);
    if state.identity.market_id.to_bytes() != request.market
        || state.identity.generation != request.generation
        || state.identity.registry_program.to_bytes() != direct.registry_program.key.to_bytes()
        || state.identity.selected_release_set.to_bytes() != request.release_set
        || state.identity.resolution_policy.to_bytes() != request.role.source_material
        || state.identity.capability_manifest.to_bytes() != request.role.capability_manifest
        || state.phase != CorePhase::Retiring
        || state.readiness != CoreReadiness::Consumed
        || state.terminal_receipt.map(|value| value.to_bytes())
            != Some(direct.certificate.key.to_bytes())
        || state.rent_beneficiary.to_bytes() != request.role.beneficiary
        || Pubkey::find_program_address(&seeds.as_slices(), direct.core_program.key).0
            != *direct.market.key
    {
        return Err(ResolutionError::MarketAuthority.into());
    }
    Ok(state)
}

fn authenticate_direct_close_release(
    program_id: &Pubkey,
    direct: DirectCloseAccounts<'_, '_>,
    request: &DirectFundingCloseRequestV1,
) -> ProgramResult {
    if direct.activated_release_set.owner != direct.registry_program.key {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    let activation_data = direct
        .activated_release_set
        .try_borrow_data()
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    if activation_data.len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1 {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&activation_data)
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    let release_set_id = activated
        .execution_release_set_id()
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    let core = activated
        .role(ExecutionRoleV1::Core)
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    let resolution = activated
        .role(ExecutionRoleV1::Resolution)
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    if release_set_id.to_bytes() != request.release_set
        || Pubkey::find_program_address(
            &[ACTIVATION_PDA_DOMAIN_V1, release_set_id.as_bytes()],
            direct.registry_program.key,
        )
        .0 != *direct.activated_release_set.key
        || core.release().program().to_bytes() != direct.core_program.key.to_bytes()
        || resolution.release().program().to_bytes() != program_id.to_bytes()
        || resolution.release().semantic_release_id().to_bytes()
            != RESOLUTION_CONTROLLER_RELEASE_ID_V7
    {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    core.authenticate_current_deployment(deployment_observation(
        direct.core_program,
        direct.core_programdata,
        core.release().programdata(),
    )?)
    .map_err(|_| ResolutionError::ResolutionDeployment)?;
    resolution
        .authenticate_current_deployment(deployment_observation(
            direct.resolution_program,
            direct.resolution_programdata,
            resolution.release().programdata(),
        )?)
        .map_err(|_| ResolutionError::ResolutionDeployment.into())
}

fn authenticate_direct_close_records(
    direct: DirectCloseAccounts<'_, '_>,
    request: &DirectFundingCloseRequestV1,
    rent: &Rent,
) -> ProgramResult {
    let material_data = direct
        .source_material
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_finalized_record(
        *direct.registry_program.key,
        direct.source_material,
        direct.source_material_staging,
        rent,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        request.role.source_material,
        &material_data,
        RecordKind::SourceMaterialV3,
    )?;
    let manifest_data = direct
        .capability_manifest
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_finalized_record(
        *direct.registry_program.key,
        direct.capability_manifest,
        direct.capability_manifest_staging,
        rent,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        request.role.capability_manifest,
        &manifest_data,
        RecordKind::CapabilityManifest,
    )
}

fn authenticate_direct_close_recovery_policy(
    direct: DirectCloseAccounts<'_, '_>,
    raw: Option<&AccountInfo<'_>>,
    staging: Option<&AccountInfo<'_>>,
    material: SourceMaterialV3,
    rent: &Rent,
) -> Result<Option<RecoveryPolicyV2>, ProgramError> {
    match (material.recovery_policy(), raw, staging) {
        (Some(policy_id), Some(raw), Some(staging)) => {
            let policy_data = raw
                .try_borrow_data()
                .map_err(|_| ResolutionError::FinalizedRecord)?;
            authenticate_finalized_record(
                *direct.registry_program.key,
                raw,
                staging,
                rent,
                RECOVERY_POLICY_SCHEMA_ID_V2,
                policy_id.to_bytes(),
                &policy_data,
                RecordKind::RecoveryPolicyV2,
            )?;
            RecoveryPolicyV2::decode(&policy_data)
                .map(Some)
                .map_err(|_| ResolutionError::SourceMaterial.into())
        }
        (None, None, None) => Ok(None),
        _ => Err(ResolutionError::AccountFrame.into()),
    }
}

#[allow(clippy::too_many_arguments)]
fn authenticate_direct_close_ledger(
    program_id: &Pubkey,
    direct: DirectCloseAccounts<'_, '_>,
    generation: u64,
    manifest_id: CapabilityContentId,
    manifest: CapabilityManifestV1<'_>,
    request: ResolutionRoleRequestV2,
    expected_status: FundingLedgerStatusV2,
    bytes: &[u8],
    observed_lamports: u64,
    rent: &Rent,
    admit_donations: bool,
) -> ProgramResult {
    if direct.funding_ledger.owner != program_id
        || direct.funding_ledger.data_len() != RESOLUTION_FUNDING_LEDGER_BYTES
    {
        return Err(ResolutionError::Funding.into());
    }
    authenticate_ledger_value(
        program_id,
        direct.market,
        direct.funding_ledger,
        generation,
        manifest_id,
        manifest,
        request,
        expected_status,
        bytes,
        observed_lamports,
        rent,
        admit_donations,
    )
}

fn authenticate_direct_market(
    direct: DirectFundingAccounts<'_, '_>,
    request: &FundingActivationRequestV1,
) -> Result<CoreState, ProgramError> {
    if direct.market.owner != direct.core_program.key {
        return Err(ResolutionError::MarketAuthority.into());
    }
    let market_data = direct
        .market
        .try_borrow_data()
        .map_err(|_| ResolutionError::MarketAuthority)?;
    if hash(&market_data).to_bytes() != request.expected_market_state_digest {
        return Err(ResolutionError::MarketAuthority.into());
    }
    let state = CoreState::decode(&market_data).map_err(|_| ResolutionError::MarketAuthority)?;
    let seeds = MarketCoreStateSeedsV2::new(state.identity);
    if state.identity.market_id.to_bytes() != request.market
        || state.identity.generation != request.generation
        || state.identity.registry_program.to_bytes() != direct.registry_program.key.to_bytes()
        || state.identity.selected_release_set.to_bytes() != request.release_set
        || state.identity.resolution_policy.to_bytes() != request.role.source_material
        || state.identity.capability_manifest.to_bytes() != request.role.capability_manifest
        || state.terminal_receipt.is_some()
        || !matches!(
            (state.phase, state.readiness),
            (CorePhase::Founding, CoreReadiness::Prepaid)
                | (CorePhase::Open, CoreReadiness::Consumed)
        )
        || Pubkey::find_program_address(&seeds.as_slices(), direct.core_program.key).0
            != *direct.market.key
    {
        return Err(ResolutionError::MarketAuthority.into());
    }
    Ok(state)
}

fn authenticate_direct_activation(
    program_id: &Pubkey,
    direct: DirectFundingAccounts<'_, '_>,
    request: &FundingActivationRequestV1,
) -> ProgramResult {
    if direct.activated_release_set.owner != direct.registry_program.key {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    let activation_data = direct
        .activated_release_set
        .try_borrow_data()
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    if activation_data.len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1 {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&activation_data)
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    let release_set_id = activated
        .execution_release_set_id()
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    let core = activated
        .role(ExecutionRoleV1::Core)
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    let resolution = activated
        .role(ExecutionRoleV1::Resolution)
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    if release_set_id.to_bytes() != request.release_set
        || Pubkey::find_program_address(
            &[ACTIVATION_PDA_DOMAIN_V1, release_set_id.as_bytes()],
            direct.registry_program.key,
        )
        .0 != *direct.activated_release_set.key
        || core.release().program().to_bytes() != direct.core_program.key.to_bytes()
        || resolution.release().program().to_bytes() != program_id.to_bytes()
        || resolution.release().semantic_release_id().to_bytes()
            != RESOLUTION_CONTROLLER_RELEASE_ID_V7
    {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    core.authenticate_current_deployment(deployment_observation(
        direct.core_program,
        direct.core_programdata,
        core.release().programdata(),
    )?)
    .map_err(|_| ResolutionError::ResolutionDeployment)?;
    resolution
        .authenticate_current_deployment(deployment_observation(
            direct.resolution_program,
            direct.resolution_programdata,
            resolution.release().programdata(),
        )?)
        .map_err(|_| ResolutionError::ResolutionDeployment.into())
}

fn authenticate_direct_source_records(
    direct: DirectFundingAccounts<'_, '_>,
    request: &FundingActivationRequestV1,
    rent: &Rent,
) -> ProgramResult {
    let material_data = direct
        .source_material
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_finalized_record(
        *direct.registry_program.key,
        direct.source_material,
        direct.source_material_staging,
        rent,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        request.role.source_material,
        &material_data,
        RecordKind::SourceMaterialV3,
    )?;
    let manifest_data = direct
        .capability_manifest
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_finalized_record(
        *direct.registry_program.key,
        direct.capability_manifest,
        direct.capability_manifest_staging,
        rent,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        request.role.capability_manifest,
        &manifest_data,
        RecordKind::CapabilityManifest,
    )
}

fn authenticate_direct_recovery_policy(
    direct: DirectFundingAccounts<'_, '_>,
    raw: Option<&AccountInfo<'_>>,
    staging: Option<&AccountInfo<'_>>,
    material: SourceMaterialV3,
    rent: &Rent,
) -> Result<Option<RecoveryPolicyV2>, ProgramError> {
    match (material.recovery_policy(), raw, staging) {
        (Some(policy_id), Some(raw), Some(staging)) => {
            let policy_data = raw
                .try_borrow_data()
                .map_err(|_| ResolutionError::FinalizedRecord)?;
            authenticate_finalized_record(
                *direct.registry_program.key,
                raw,
                staging,
                rent,
                RECOVERY_POLICY_SCHEMA_ID_V2,
                policy_id.to_bytes(),
                &policy_data,
                RecordKind::RecoveryPolicyV2,
            )?;
            RecoveryPolicyV2::decode(&policy_data)
                .map(Some)
                .map_err(|_| ResolutionError::SourceMaterial.into())
        }
        (None, None, None) => Ok(None),
        _ => Err(ResolutionError::AccountFrame.into()),
    }
}

fn authenticate_direct_source(
    program_id: &Pubkey,
    direct: DirectFundingAccounts<'_, '_>,
    request: &FundingActivationRequestV1,
    state: CoreState,
) -> ProgramResult {
    let source_data = direct
        .source_state
        .try_borrow_data()
        .map_err(|_| ResolutionError::OutputState)?;
    if hash(&source_data).to_bytes() != request.expected_source_state_digest {
        return Err(ResolutionError::Transition.into());
    }
    let source =
        SourceResolutionStateV2::decode(&source_data).map_err(|_| ResolutionError::OutputState)?;
    authenticate_state_account_v2(program_id, direct.source_state, source)?;
    if source.phase() != SourceResolutionPhaseV1::Primary
        || source.market() != request.market
        || source.generation() != request.generation
        || source.material_id().to_bytes() != request.role.source_material
        || source.rent_beneficiary() != state.rent_beneficiary.to_bytes()
        || source.rent_beneficiary() != request.role.beneficiary
    {
        return Err(ResolutionError::Transition.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_direct_ledger(
    program_id: &Pubkey,
    direct: DirectFundingAccounts<'_, '_>,
    generation: u64,
    manifest_id: CapabilityContentId,
    manifest: CapabilityManifestV1<'_>,
    request: ResolutionRoleRequestV2,
    expected_status: FundingLedgerStatusV2,
    bytes: &[u8],
    observed_lamports: u64,
    rent: &Rent,
    admit_donations: bool,
) -> ProgramResult {
    if direct.funding_ledger.owner != program_id
        || direct.funding_ledger.data_len() != RESOLUTION_FUNDING_LEDGER_BYTES
    {
        return Err(ResolutionError::Funding.into());
    }
    authenticate_ledger_value(
        program_id,
        direct.market,
        direct.funding_ledger,
        generation,
        manifest_id,
        manifest,
        request,
        expected_status,
        bytes,
        observed_lamports,
        rent,
        admit_donations,
    )
}

#[allow(clippy::too_many_arguments)]
fn authenticate_completed_activation(
    program_id: &Pubkey,
    direct: DirectFundingAccounts<'_, '_>,
    request: FundingActivationRequestV1,
    request_digest: [u8; 32],
    manifest_id: CapabilityContentId,
    manifest: CapabilityManifestV1<'_>,
    rent: &Rent,
) -> ProgramResult {
    let receipt_data = direct
        .receipt
        .try_borrow_data()
        .map_err(|_| ResolutionError::OutputState)?;
    if direct.receipt.data_len() != FUNDING_ACTIVATION_RECEIPT_BYTES_V1
        || direct.receipt.lamports() < rent.minimum_balance(FUNDING_ACTIVATION_RECEIPT_BYTES_V1)
    {
        return Err(ResolutionError::OutputState.into());
    }
    let receipt = FundingActivationReceiptV1::decode(&receipt_data)
        .map_err(|_| ResolutionError::OutputState)?;
    let ledger_bytes = copy_ledger_bytes(direct.funding_ledger)?;
    authenticate_direct_ledger(
        program_id,
        direct,
        request.generation,
        manifest_id,
        manifest,
        request.role,
        FundingLedgerStatusV2::Active,
        &ledger_bytes,
        direct.funding_ledger.lamports(),
        rent,
        false,
    )?;
    let active_digest = funding_lifecycle_account_digest_v1(
        program_id.to_bytes(),
        direct.funding_ledger.key.to_bytes(),
        direct.funding_ledger.lamports(),
        &ledger_bytes,
    );
    if receipt.request_digest != request_digest
        || receipt.release_set != request.release_set
        || receipt.resolution_release != RESOLUTION_CONTROLLER_RELEASE_ID_V7
        || receipt.market != request.market
        || receipt.generation != request.generation
        || receipt.role != request.role
        || receipt.market_state_digest != request.expected_market_state_digest
        || receipt.source_state_digest != request.expected_source_state_digest
        || receipt.pending_ledger_digest != request.expected_pending_ledger_digest
        || receipt.active_ledger_digest != active_digest
        || receipt.post_ledger_lamports != direct.funding_ledger.lamports()
        || receipt.producer != program_id.to_bytes()
    {
        return Err(ResolutionError::Funding.into());
    }
    set_return_data(&receipt_data);
    Ok(())
}

fn initialize_activation_receipt<'info>(
    program_id: &Pubkey,
    market: &AccountInfo<'info>,
    output: &AccountInfo<'info>,
    generation: u64,
    system: &AccountInfo<'info>,
    rent: &Rent,
) -> ProgramResult {
    let generation_seed = generation.to_le_bytes();
    let (expected, bump) = Pubkey::find_program_address(
        &[
            FUNDING_ACTIVATION_RECEIPT_PDA_DOMAIN_V1,
            market.key.as_ref(),
            &generation_seed,
        ],
        program_id,
    );
    if output.key != &expected {
        return Err(ResolutionError::OutputState.into());
    }
    require_prepaid_output(
        output,
        rent.minimum_balance(FUNDING_ACTIVATION_RECEIPT_BYTES_V1),
    )?;
    let bump_seed = [bump];
    let signer: [&[u8]; 4] = [
        FUNDING_ACTIVATION_RECEIPT_PDA_DOMAIN_V1,
        market.key.as_ref(),
        &generation_seed,
        &bump_seed,
    ];
    let space = u64::try_from(FUNDING_ACTIVATION_RECEIPT_BYTES_V1)
        .map_err(|_| ResolutionError::Arithmetic)?;
    invoke_signed(
        &allocate(output.key, space),
        &[output.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| ResolutionError::OutputState)?;
    invoke_signed(
        &assign(output.key, program_id),
        &[output.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| ResolutionError::OutputState)?;
    if output.owner != program_id
        || output.data_len() != FUNDING_ACTIVATION_RECEIPT_BYTES_V1
        || output.lamports() < rent.minimum_balance(FUNDING_ACTIVATION_RECEIPT_BYTES_V1)
    {
        return Err(ResolutionError::OutputState.into());
    }
    Ok(())
}

/// Execute one Core-owned envelope through the sole Resolution semantic request.
#[inline(never)]
pub(crate) fn process_core_effect(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if !is_core_effect(instruction_data) {
        return Err(ResolutionError::Instruction.into());
    }
    let envelope_bytes = instruction_data
        .get(..CORE_EFFECT_ENVELOPE_BYTES_V1)
        .ok_or(ResolutionError::Instruction)?;
    let role_bytes = instruction_data
        .get(CORE_EFFECT_ENVELOPE_BYTES_V1..)
        .ok_or(ResolutionError::Instruction)?;
    let funding_header_bytes = role_bytes
        .get(..CAPABILITY_FUNDING_HEADER_BYTES_V2)
        .ok_or(ResolutionError::Instruction)?;
    let request_bytes = role_bytes
        .get(CAPABILITY_FUNDING_HEADER_BYTES_V2..)
        .ok_or(ResolutionError::Instruction)?;
    if request_bytes.len() != RESOLUTION_CORE_ROLE_REQUEST_BYTES_V2 {
        return Err(ResolutionError::Instruction.into());
    }
    let funding_header = CapabilityFundingHeaderV2::decode(funding_header_bytes)
        .map_err(|_| ResolutionError::Instruction)?;
    let envelope =
        CoreEffectEnvelopeV1::decode(envelope_bytes).map_err(|_| ResolutionError::Instruction)?;
    let request =
        ResolutionRoleRequestV2::decode(request_bytes).map_err(|_| ResolutionError::Instruction)?;
    if matches!(
        request.action,
        ResolutionCoreActionV1::VerifyFundReady | ResolutionCoreActionV1::CloseFund
    ) {
        // V7 owns both mutations as direct permissionless routes. No Core PDA
        // can re-enable the superseded composed paths.
        return Err(ResolutionError::Instruction.into());
    }
    authenticate_action(envelope, request)?;
    authenticate_funding_header(funding_header, request)?;
    let expected_accounts = match request.action {
        ResolutionCoreActionV1::CreateFund => CREATE_FUND_ACCOUNT_COUNT,
        ResolutionCoreActionV1::VerifyFundReady => VERIFY_FUND_ACCOUNT_COUNT,
        ResolutionCoreActionV1::AdmitTerminal => ADMIT_TERMINAL_ACCOUNT_COUNT,
        ResolutionCoreActionV1::CloseFund => CLOSE_FUND_ACCOUNT_COUNT,
    };
    // The three fund actions end with the finalized RecoveryPolicyV2 pair. A
    // material that bought no recovery walk has no such record, so its frame
    // is the same frame without those two tail positions; whether the short
    // shape is admissible is decided against the authenticated material in
    // `authenticate_recovery_policy`, not here.
    let admissible_count = accounts.len() == expected_accounts
        || (request.action != ResolutionCoreActionV1::AdmitTerminal
            && accounts.len() == expected_accounts.saturating_sub(2));
    if !admissible_count {
        return Err(ResolutionError::AccountFrame.into());
    }
    let common = parse_common(accounts)?;
    authenticate_common_frame(program_id, accounts, common, request)?;
    let rent_account = accounts
        .get(match request.action {
            ResolutionCoreActionV1::CreateFund => 14,
            ResolutionCoreActionV1::VerifyFundReady => 16,
            ResolutionCoreActionV1::AdmitTerminal => 15,
            ResolutionCoreActionV1::CloseFund => 18,
        })
        .ok_or(ResolutionError::AccountFrame)?;
    let rent = authenticate_rent(rent_account)?;
    let authenticated = authenticate_core(
        program_id,
        common,
        envelope,
        request,
        envelope_bytes,
        role_bytes,
        &rent,
    )?;
    match request.action {
        ResolutionCoreActionV1::CreateFund => process_create(
            program_id,
            accounts,
            common,
            envelope,
            request,
            authenticated,
            &rent,
        ),
        ResolutionCoreActionV1::VerifyFundReady => process_verify(
            program_id,
            accounts,
            common,
            envelope,
            request,
            authenticated,
            &rent,
        ),
        ResolutionCoreActionV1::AdmitTerminal => process_admit(
            program_id,
            accounts,
            common,
            envelope,
            request,
            authenticated,
            &rent,
        ),
        ResolutionCoreActionV1::CloseFund => process_close(
            program_id,
            accounts,
            common,
            &envelope,
            &request,
            &authenticated,
            &rent,
        ),
    }
}

fn parse_common<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
) -> Result<CommonAccounts<'a, 'info>, ProgramError> {
    let mut iterator = accounts.iter();
    Ok(CommonAccounts {
        caller_authority: next(&mut iterator)?,
        market: next(&mut iterator)?,
        activated_release_set: next(&mut iterator)?,
        registry_program: next(&mut iterator)?,
        core_program: next(&mut iterator)?,
        core_programdata: next(&mut iterator)?,
        resolution_program: next(&mut iterator)?,
        resolution_programdata: next(&mut iterator)?,
        source_material: next(&mut iterator)?,
        source_material_staging: next(&mut iterator)?,
        capability_manifest: next(&mut iterator)?,
        capability_manifest_staging: next(&mut iterator)?,
        source_state: next(&mut iterator)?,
        funding_ledger: next(&mut iterator)?,
    })
}

fn authenticate_action(
    envelope: CoreEffectEnvelopeV1,
    request: ResolutionRoleRequestV2,
) -> ProgramResult {
    let expected = match request.action {
        ResolutionCoreActionV1::CreateFund => CoreEffectActionV1::CreateFund,
        ResolutionCoreActionV1::VerifyFundReady => CoreEffectActionV1::VerifyFundReady,
        ResolutionCoreActionV1::AdmitTerminal => CoreEffectActionV1::AdmitTerminal,
        ResolutionCoreActionV1::CloseFund => CoreEffectActionV1::CloseFund,
    };
    if envelope.action() != expected || envelope.target_role() != Role::Resolution {
        return Err(ResolutionError::Instruction.into());
    }
    Ok(())
}

fn authenticate_funding_header(
    funding_header: CapabilityFundingHeaderV2,
    request: ResolutionRoleRequestV2,
) -> ProgramResult {
    if funding_header.physical_count() == 1
        && funding_header.logical_count() == 3
        && funding_header.selected_mask()
            == request
                .funding_entry_mask()
                .map_err(|_| ResolutionError::Funding)?
    {
        Ok(())
    } else {
        Err(ResolutionError::Instruction.into())
    }
}

fn authenticate_common_frame(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    common: CommonAccounts<'_, '_>,
    request: ResolutionRoleRequestV2,
) -> ProgramResult {
    if common.resolution_program.key != program_id
        || !common.caller_authority.is_signer
        || common.caller_authority.is_writable
        || !common.registry_program.executable
        || !common.core_program.executable
        || !common.resolution_program.executable
        || common.source_material.is_writable
        || common.source_material_staging.is_writable
        || common.capability_manifest.is_writable
        || common.capability_manifest_staging.is_writable
        || common.market.is_writable
        || common.activated_release_set.is_writable
        || common.core_programdata.is_writable
        || common.resolution_programdata.is_writable
        || common.source_material.executable
        || common.source_material_staging.executable
        || common.capability_manifest.executable
        || common.capability_manifest_staging.executable
        || common.market.executable
        || common.activated_release_set.executable
        || common.core_programdata.executable
        || common.resolution_programdata.executable
        || common.source_state.key.to_bytes() != request.source_state
        || common.funding_ledger.key.to_bytes() != request.funding_ledger
    {
        return Err(ResolutionError::AccountFrame.into());
    }
    for (index, account) in accounts.iter().enumerate() {
        if account.is_signer != (index == 0)
            || accounts
                .iter()
                .skip(index.checked_add(1).ok_or(ResolutionError::Arithmetic)?)
                .any(|other| other.key == account.key)
        {
            return Err(ResolutionError::AccountFrame.into());
        }
    }
    let writable = match request.action {
        ResolutionCoreActionV1::CreateFund => [true, false],
        ResolutionCoreActionV1::VerifyFundReady => [false, true],
        ResolutionCoreActionV1::AdmitTerminal => [false, false],
        ResolutionCoreActionV1::CloseFund => [true, true],
    };
    for (account, expected) in [common.source_state, common.funding_ledger]
        .into_iter()
        .zip(writable)
    {
        if account.is_writable != expected || account.executable {
            return Err(ResolutionError::AccountFrame.into());
        }
    }
    let tail_profile: &[(bool, bool)] = match request.action {
        ResolutionCoreActionV1::CreateFund => &[
            (false, false),
            (false, true),
            (false, false),
            (false, false),
        ],
        ResolutionCoreActionV1::VerifyFundReady => &[
            (true, false),
            (false, false),
            (false, false),
            (false, false),
            (false, false),
        ],
        ResolutionCoreActionV1::AdmitTerminal => &[
            (false, false),
            (false, false),
            (false, false),
            (false, false),
            (false, false),
            (false, false),
            (false, false),
            (false, false),
        ],
        ResolutionCoreActionV1::CloseFund => &[
            (false, false),
            (true, false),
            (true, false),
            (false, false),
            (false, false),
            (false, true),
            (false, false),
            (false, false),
        ],
    };
    for (account, (writable, executable)) in accounts.iter().skip(14).zip(tail_profile.iter()) {
        if account.is_writable != *writable || account.executable != *executable {
            return Err(ResolutionError::AccountFrame.into());
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_core(
    program_id: &Pubkey,
    common: CommonAccounts<'_, '_>,
    envelope: CoreEffectEnvelopeV1,
    request: ResolutionRoleRequestV2,
    envelope_bytes: &[u8],
    role_bytes: &[u8],
    rent: &Rent,
) -> Result<AuthenticatedCore, ProgramError> {
    let request_digest =
        Identity::new(hash(role_bytes).to_bytes()).map_err(|_| ResolutionError::Instruction)?;
    envelope
        .validate_role_request(role_bytes.len(), request_digest)
        .map_err(|_| ResolutionError::Instruction)?;
    if envelope.caller_program().to_bytes() != common.core_program.key.to_bytes()
        || envelope.caller_authority().to_bytes() != common.caller_authority.key.to_bytes()
        || envelope.market().to_bytes() != common.market.key.to_bytes()
        || envelope.context().to_bytes() != request.source_state
    {
        return Err(ResolutionError::MarketAuthority.into());
    }
    let caller_seeds = envelope
        .caller_authority_seeds()
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    let expected_caller =
        Pubkey::find_program_address(&caller_seeds.as_slices(), common.core_program.key).0;
    if common.caller_authority.key != &expected_caller {
        return Err(ResolutionError::ResolutionRelease.into());
    }

    if common.market.owner != common.core_program.key || common.market.executable {
        return Err(ResolutionError::MarketAuthority.into());
    }
    let market_data = common
        .market
        .try_borrow_data()
        .map_err(|_| ResolutionError::MarketAuthority)?;
    let state = CoreState::decode(&market_data).map_err(|_| ResolutionError::MarketAuthority)?;
    let state_digest = Identity::new(hash(&market_data).to_bytes())
        .map_err(|_| ResolutionError::MarketAuthority)?;
    if envelope.parent_state_digest() != state_digest
        || state.identity.market_id.to_bytes() != common.market.key.to_bytes()
        || state.identity.registry_program.to_bytes() != common.registry_program.key.to_bytes()
        || state.identity.resolution_policy.to_bytes() != request.source_material
        || state.identity.capability_manifest.to_bytes() != request.capability_manifest
        || state.identity.selected_release_set.to_bytes() != envelope.release_set().to_bytes()
        || state.identity.generation != envelope.generation()
    {
        return Err(ResolutionError::MarketAuthority.into());
    }
    let market_seeds = MarketCoreStateSeedsV2::new(state.identity);
    if Pubkey::find_program_address(&market_seeds.as_slices(), common.core_program.key).0
        != *common.market.key
    {
        return Err(ResolutionError::MarketAuthority.into());
    }
    match request.action {
        ResolutionCoreActionV1::CreateFund | ResolutionCoreActionV1::VerifyFundReady => {
            // Both founding routes, and only before a terminal receipt exists.
            // `Founding + Prepaid` is the readiness ladder. `Open + Consumed`
            // is the atomic founding, whose commit-last Open goes from the
            // first straight to the second in one transition and therefore
            // never passes the ladder; without this arm every atomically
            // founded Market is permanently unresolvable, because this is the
            // only route that creates a `SourceResolutionStateV2`.
            //
            // Deferring the Source state's physical creation past Open defers
            // no decision: the manifest is a seed of the Market address and
            // the Resolution subset ledger was already initialized before
            // Market Found. This route only consumes that immutable authority.
            if state.terminal_receipt.is_some()
                || !matches!(
                    (state.phase, state.readiness),
                    (CorePhase::Founding, CoreReadiness::Prepaid)
                        | (CorePhase::Open, CoreReadiness::Consumed)
                )
            {
                return Err(ResolutionError::MarketAuthority.into());
            }
        }
        ResolutionCoreActionV1::AdmitTerminal => {
            if state.phase != CorePhase::Open || state.readiness != CoreReadiness::Consumed {
                return Err(ResolutionError::MarketAuthority.into());
            }
        }
        ResolutionCoreActionV1::CloseFund => {
            if state.phase != CorePhase::Retiring || state.readiness != CoreReadiness::Consumed {
                return Err(ResolutionError::MarketAuthority.into());
            }
        }
    }

    authenticate_activation(program_id, common, envelope)?;
    authenticate_source_records(common, state, request, rent)?;
    let envelope_len = u32::try_from(envelope_bytes.len())
        .map_err(|_| ResolutionError::Arithmetic)?
        .to_le_bytes();
    let request_len = u32::try_from(role_bytes.len())
        .map_err(|_| ResolutionError::Arithmetic)?
        .to_le_bytes();
    let full_effect_digest = Identity::new(
        hashv(&[
            &CORE_EFFECT_DIGEST_DOMAIN_V1,
            &envelope_len,
            envelope_bytes,
            &request_len,
            role_bytes,
        ])
        .to_bytes(),
    )
    .map_err(|_| ResolutionError::Instruction)?;
    Ok(AuthenticatedCore {
        state,
        full_effect_digest,
    })
}

fn authenticate_activation(
    program_id: &Pubkey,
    common: CommonAccounts<'_, '_>,
    envelope: CoreEffectEnvelopeV1,
) -> ProgramResult {
    if common.activated_release_set.owner != common.registry_program.key
        || common.activated_release_set.executable
    {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    let activation_data = common
        .activated_release_set
        .try_borrow_data()
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    if activation_data.len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1 {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&activation_data)
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    let release_set_id = activated
        .execution_release_set_id()
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    if release_set_id.to_bytes() != envelope.release_set().to_bytes()
        || Pubkey::find_program_address(
            &[ACTIVATION_PDA_DOMAIN_V1, release_set_id.as_bytes()],
            common.registry_program.key,
        )
        .0 != *common.activated_release_set.key
    {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    let core = activated
        .role(ExecutionRoleV1::Core)
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    let resolution = activated
        .role(ExecutionRoleV1::Resolution)
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    if core.release().program().to_bytes() != common.core_program.key.to_bytes()
        || resolution.release().program().to_bytes() != program_id.to_bytes()
        || resolution.release().semantic_release_id().to_bytes()
            != RESOLUTION_CONTROLLER_RELEASE_ID_V7
    {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    let core_observation = deployment_observation(
        common.core_program,
        common.core_programdata,
        core.release().programdata(),
    )?;
    core.authenticate_current_deployment(core_observation)
        .map_err(|_| ResolutionError::ResolutionDeployment)?;
    let resolution_observation = deployment_observation(
        common.resolution_program,
        common.resolution_programdata,
        resolution.release().programdata(),
    )?;
    resolution
        .authenticate_current_deployment(resolution_observation)
        .map_err(|_| ResolutionError::ResolutionDeployment.into())
}

fn authenticate_source_records(
    common: CommonAccounts<'_, '_>,
    state: CoreState,
    request: ResolutionRoleRequestV2,
    rent: &Rent,
) -> ProgramResult {
    let material_data = common
        .source_material
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_finalized_record(
        *common.registry_program.key,
        common.source_material,
        common.source_material_staging,
        rent,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        request.source_material,
        &material_data,
        RecordKind::SourceMaterialV3,
    )?;
    let material =
        SourceMaterialV3::decode(&material_data).map_err(|_| ResolutionError::SourceMaterial)?;
    if material.product_record_digest().to_bytes() != state.identity.product_record.to_bytes() {
        return Err(ResolutionError::SourceMaterial.into());
    }
    let manifest_data = common
        .capability_manifest
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_finalized_record(
        *common.registry_program.key,
        common.capability_manifest,
        common.capability_manifest_staging,
        rent,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        request.capability_manifest,
        &manifest_data,
        RecordKind::CapabilityManifest,
    )?;
    CapabilityManifestV1::decode(&manifest_data).map_err(|_| ResolutionError::Funding)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn process_create<'info>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'info>],
    common: CommonAccounts<'_, 'info>,
    envelope: CoreEffectEnvelopeV1,
    request: ResolutionRoleRequestV2,
    authenticated: AuthenticatedCore,
    rent: &Rent,
) -> ProgramResult {
    require_revisions(&envelope, 0, 0)?;
    let system = accounts.get(15).ok_or(ResolutionError::AccountFrame)?;
    if system.key != &system_program::ID
        || !system.executable
        || system.is_writable
        || request.beneficiary != authenticated.state.rent_beneficiary.to_bytes()
    {
        return Err(ResolutionError::AccountFrame.into());
    }
    let material_data = common
        .source_material
        .try_borrow_data()
        .map_err(|_| ResolutionError::SourceMaterial)?;
    let material =
        SourceMaterialV3::decode(&material_data).map_err(|_| ResolutionError::SourceMaterial)?;
    let recovery_policy =
        authenticate_recovery_policy(common, accounts.get(16), accounts.get(17), material, rent)?;
    let manifest_data = common
        .capability_manifest
        .try_borrow_data()
        .map_err(|_| ResolutionError::Funding)?;
    let manifest =
        CapabilityManifestV1::decode(&manifest_data).map_err(|_| ResolutionError::Funding)?;
    authenticate_funding_entries(material, recovery_policy, manifest, request)?;
    let manifest_id = CapabilityContentId::new(request.capability_manifest)
        .map_err(|_| ResolutionError::Funding)?;

    let (expected_source, source_bump) = Pubkey::find_program_address(
        &[
            dclutch_source_contract::SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
            common.market.key.as_ref(),
            &authenticated.state.identity.generation.to_le_bytes(),
        ],
        program_id,
    );
    if common.source_state.key != &expected_source {
        return Err(ResolutionError::OutputState.into());
    }
    require_prepaid_output(
        common.source_state,
        rent.minimum_balance(SOURCE_RESOLUTION_STATE_BYTES_V2),
    )?;
    let source_plan = SourceResolutionStateV2::fresh(
        common.market.key.to_bytes(),
        authenticated.state.identity.generation,
        dclutch_source_contract::ContentId::new(request.source_material)
            .map_err(|_| ResolutionError::SourceMaterial)?,
        request.beneficiary,
        source_bump,
        0,
        0,
    )
    .map_err(|_| ResolutionError::Transition)?;
    let source = source_plan.state();

    // V6 has exactly one ledger initialization authority: the pre-Market
    // initializer. CreateFund consumes that already-owned Pending ledger and
    // must not allocate, assign, rewrite, or change its aggregate custody.
    let ledger_bytes = copy_ledger_bytes(common.funding_ledger)?;
    let ledger_lamports = common.funding_ledger.lamports();
    authenticate_live_ledger(
        program_id,
        common,
        authenticated.state.identity.generation,
        manifest_id,
        manifest,
        request,
        FundingLedgerStatusV2::Pending,
        &ledger_bytes,
        ledger_lamports,
        rent,
        false,
    )?;
    let source_bytes = source.to_bytes();
    let post_digest = poststate_digest(request.action, &source_bytes, &ledger_bytes, None)?;

    initialize_source_output(
        program_id,
        common.market,
        common.source_state,
        system,
        authenticated.state.identity.generation,
        source_bump,
        rent,
    )?;
    drop(manifest_data);
    drop(material_data);
    write_state(common.source_state, &source_bytes)?;
    let observed_ledger = common
        .funding_ledger
        .try_borrow_data()
        .map_err(|_| ResolutionError::OutputState)?;
    if observed_ledger.as_ref() != ledger_bytes
        || common.funding_ledger.lamports() != ledger_lamports
    {
        return Err(ResolutionError::OutputState.into());
    }
    drop(observed_ledger);
    return_ack(
        program_id,
        &envelope,
        authenticated.full_effect_digest,
        post_digest,
        0,
        0,
        0,
        0,
    )
}

#[allow(clippy::too_many_arguments)]
fn process_verify(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    common: CommonAccounts<'_, '_>,
    envelope: CoreEffectEnvelopeV1,
    request: ResolutionRoleRequestV2,
    authenticated: AuthenticatedCore,
    rent: &Rent,
) -> ProgramResult {
    require_revisions(&envelope, 0, 0)?;
    let beneficiary = accounts.get(14).ok_or(ResolutionError::AccountFrame)?;
    let clock_account = accounts.get(15).ok_or(ResolutionError::AccountFrame)?;
    if beneficiary.key.to_bytes() != request.beneficiary
        || request.beneficiary != authenticated.state.rent_beneficiary.to_bytes()
        || !beneficiary.is_writable
        || beneficiary.executable
        || clock_account.is_writable
    {
        return Err(ResolutionError::AccountFrame.into());
    }
    let clock = authenticate_clock(clock_account)?;
    if clock.slot == 0 {
        return Err(ResolutionError::Sysvar.into());
    }
    let material_data = common
        .source_material
        .try_borrow_data()
        .map_err(|_| ResolutionError::SourceMaterial)?;
    let material =
        SourceMaterialV3::decode(&material_data).map_err(|_| ResolutionError::SourceMaterial)?;
    let recovery_policy =
        authenticate_recovery_policy(common, accounts.get(17), accounts.get(18), material, rent)?;
    let manifest_data = common
        .capability_manifest
        .try_borrow_data()
        .map_err(|_| ResolutionError::Funding)?;
    let manifest =
        CapabilityManifestV1::decode(&manifest_data).map_err(|_| ResolutionError::Funding)?;
    authenticate_funding_entries(material, recovery_policy, manifest, request)?;
    let manifest_id = CapabilityContentId::new(request.capability_manifest)
        .map_err(|_| ResolutionError::Funding)?;
    let source_bytes = common
        .source_state
        .try_borrow_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let source =
        SourceResolutionStateV2::decode(&source_bytes).map_err(|_| ResolutionError::OutputState)?;
    authenticate_state_account_v2(program_id, common.source_state, source)?;
    if source.phase() != SourceResolutionPhaseV1::Primary
        || source.market() != common.market.key.to_bytes()
        || source.generation() != authenticated.state.identity.generation
        || source.material_id().to_bytes() != request.source_material
        || source.rent_beneficiary() != request.beneficiary
    {
        return Err(ResolutionError::Transition.into());
    }
    let mut ledger_bytes = copy_ledger_bytes(common.funding_ledger)?;
    authenticate_live_ledger(
        program_id,
        common,
        authenticated.state.identity.generation,
        manifest_id,
        manifest,
        request,
        FundingLedgerStatusV2::Pending,
        &ledger_bytes,
        common.funding_ledger.lamports(),
        rent,
        false,
    )?;
    let mut total_debit = 0_u64;
    for entry_index in [
        request.recovery_entry_index,
        request.exhaustion_entry_index,
        request.failure_entry_index,
    ] {
        let debit = FundingLedgerV2::activate_in_place(
            &mut ledger_bytes,
            manifest_id,
            manifest,
            entry_index,
            clock.slot,
        )
        .map_err(|_| ResolutionError::Funding)?;
        total_debit = total_debit
            .checked_add(debit.rent_lamports())
            .and_then(|value| value.checked_add(debit.creation_lamports()))
            .ok_or(ResolutionError::Arithmetic)?;
    }
    let ledger_lamports = common
        .funding_ledger
        .lamports()
        .checked_sub(total_debit)
        .ok_or(ResolutionError::Arithmetic)?;
    let beneficiary_lamports = beneficiary
        .lamports()
        .checked_add(total_debit)
        .ok_or(ResolutionError::Arithmetic)?;
    authenticate_live_ledger(
        program_id,
        common,
        authenticated.state.identity.generation,
        manifest_id,
        manifest,
        request,
        FundingLedgerStatusV2::Active,
        &ledger_bytes,
        ledger_lamports,
        rent,
        false,
    )?;
    let post_digest = poststate_digest(request.action, &source_bytes, &ledger_bytes, None)?;
    drop(source_bytes);
    drop(manifest_data);
    drop(material_data);
    commit_activated_ledger(
        common.funding_ledger,
        &ledger_bytes,
        ledger_lamports,
        beneficiary,
        beneficiary_lamports,
    )?;
    return_ack(
        program_id,
        &envelope,
        authenticated.full_effect_digest,
        post_digest,
        0,
        0,
        0,
        1,
    )
}

#[allow(clippy::too_many_arguments)]
fn process_admit(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    common: CommonAccounts<'_, '_>,
    envelope: CoreEffectEnvelopeV1,
    request: ResolutionRoleRequestV2,
    authenticated: AuthenticatedCore,
    rent: &Rent,
) -> ProgramResult {
    let certificate_account = accounts.get(14).ok_or(ResolutionError::AccountFrame)?;
    if certificate_account.key.to_bytes() != request.receipt
        || certificate_account.is_writable
        || certificate_account.executable
    {
        return Err(ResolutionError::AccountFrame.into());
    }
    let material_data = common
        .source_material
        .try_borrow_data()
        .map_err(|_| ResolutionError::SourceMaterial)?;
    let material =
        SourceMaterialV3::decode(&material_data).map_err(|_| ResolutionError::SourceMaterial)?;
    let product_runtime =
        authenticate_admit_product_runtime(common, &authenticated.state, material, accounts, rent)?;
    let manifest_data = common
        .capability_manifest
        .try_borrow_data()
        .map_err(|_| ResolutionError::Funding)?;
    let manifest =
        CapabilityManifestV1::decode(&manifest_data).map_err(|_| ResolutionError::Funding)?;
    let manifest_id = CapabilityContentId::new(request.capability_manifest)
        .map_err(|_| ResolutionError::Funding)?;
    let source_data = common
        .source_state
        .try_borrow_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let source = authenticate_terminal_source(
        program_id,
        common,
        &request,
        &authenticated.state,
        &source_data,
    )?;
    let decision = source
        .decision(product_runtime.outcome_count)
        .map_err(|_| ResolutionError::Transition)?;
    if decision.terminal_sequence() != request.receipt_sequence {
        return Err(ResolutionError::Transition.into());
    }
    require_revisions(&envelope, request.receipt_sequence, 1)?;
    let ledger_bytes = copy_ledger_bytes(common.funding_ledger)?;
    authenticate_live_ledger(
        program_id,
        common,
        authenticated.state.identity.generation,
        manifest_id,
        manifest,
        request,
        FundingLedgerStatusV2::Active,
        &ledger_bytes,
        common.funding_ledger.lamports(),
        rent,
        false,
    )?;
    let certificate_data = certificate_account
        .try_borrow_data()
        .map_err(|_| ResolutionError::OutputState)?;
    authenticate_terminal_certificate_v2(
        program_id,
        common.source_state,
        certificate_account,
        request.receipt_kind,
        request.receipt_sequence,
        request.source_material,
        common.market.key.to_bytes(),
        authenticated.state.identity.product_record.to_bytes(),
        authenticated.state.identity.generation,
        decision.selector(),
        product_runtime.outcome_count,
        &certificate_data,
        rent,
    )?;
    let post_digest = poststate_digest(
        request.action,
        &source_data,
        &ledger_bytes,
        Some(&certificate_data),
    )?;
    return_ack(
        program_id,
        &envelope,
        authenticated.full_effect_digest,
        post_digest,
        request.receipt_sequence,
        request.receipt_sequence,
        1,
        1,
    )
}

#[allow(clippy::too_many_arguments)]
fn process_close<'info>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'info>],
    common: CommonAccounts<'_, 'info>,
    envelope: &CoreEffectEnvelopeV1,
    request: &ResolutionRoleRequestV2,
    authenticated: &AuthenticatedCore,
    rent: &Rent,
) -> ProgramResult {
    let certificate_account = accounts.get(14).ok_or(ResolutionError::AccountFrame)?;
    let closure_account = accounts.get(15).ok_or(ResolutionError::AccountFrame)?;
    let beneficiary = accounts.get(16).ok_or(ResolutionError::AccountFrame)?;
    let clock_account = accounts.get(17).ok_or(ResolutionError::AccountFrame)?;
    let system = accounts.get(19).ok_or(ResolutionError::AccountFrame)?;
    let expected_terminal = authenticated
        .state
        .terminal_receipt
        .ok_or(ResolutionError::MarketAuthority)?
        .to_bytes();
    if certificate_account.key.to_bytes() != expected_terminal
        || certificate_account.is_writable
        || closure_account.key.to_bytes() != request.receipt
        || !closure_account.is_writable
        || beneficiary.key.to_bytes() != request.beneficiary
        || request.beneficiary != authenticated.state.rent_beneficiary.to_bytes()
        || !beneficiary.is_writable
        || beneficiary.executable
        || clock_account.is_writable
        || system.key != &system_program::ID
        || !system.executable
        || system.is_writable
    {
        return Err(ResolutionError::AccountFrame.into());
    }
    let clock = authenticate_clock(clock_account)?;
    if clock.unix_timestamp <= 0 {
        return Err(ResolutionError::Sysvar.into());
    }
    authenticate_finalized_funding_policy(
        common,
        accounts.get(20),
        accounts.get(21),
        request,
        rent,
    )?;
    let manifest_data = common
        .capability_manifest
        .try_borrow_data()
        .map_err(|_| ResolutionError::Funding)?;
    let manifest =
        CapabilityManifestV1::decode(&manifest_data).map_err(|_| ResolutionError::Funding)?;
    let manifest_id = CapabilityContentId::new(request.capability_manifest)
        .map_err(|_| ResolutionError::Funding)?;
    let source_data = common
        .source_state
        .try_borrow_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut source = authenticate_terminal_source(
        program_id,
        common,
        request,
        &authenticated.state,
        &source_data,
    )?;
    let terminal = source
        .terminal_projection()
        .map_err(|_| ResolutionError::Transition)?;
    if terminal.selector() != authenticated.state.terminal_winner {
        return Err(ResolutionError::Transition.into());
    }
    let closure_sequence = terminal
        .terminal_sequence()
        .checked_add(1)
        .ok_or(ResolutionError::Arithmetic)?;
    if closure_sequence != request.receipt_sequence
        || source.rent_beneficiary() != request.beneficiary
    {
        return Err(ResolutionError::Transition.into());
    }
    require_revisions(envelope, terminal.terminal_sequence(), 1)?;
    source
        .retire(
            authenticated.state.identity.generation,
            clock.unix_timestamp,
            1,
            1,
        )
        .map_err(|_| ResolutionError::Transition)?;
    let ledger_prestate = copy_ledger_bytes(common.funding_ledger)?;
    authenticate_live_ledger(
        program_id,
        common,
        authenticated.state.identity.generation,
        manifest_id,
        manifest,
        *request,
        FundingLedgerStatusV2::Active,
        &ledger_prestate,
        common.funding_ledger.lamports(),
        rent,
        true,
    )?;
    let mut closed_ledger = ledger_prestate;
    let mut ledger_can_close = false;
    let mut planned_ledger_lamports = common.funding_ledger.lamports();
    let mut ledger_remaining_native_principal = 0_u64;
    let mut ledger_rent_lamports = 0_u64;
    let mut ledger_lamport_surplus = 0_u64;
    for entry_index in [
        request.recovery_entry_index,
        request.exhaustion_entry_index,
        request.failure_entry_index,
    ] {
        let plan = FundingLedgerV2::close_slot_in_place(
            &mut closed_ledger,
            manifest_id,
            manifest,
            entry_index,
            FundingLedgerCloseCustodyV2::native_only(
                planned_ledger_lamports,
                rent.minimum_balance(RESOLUTION_FUNDING_LEDGER_BYTES),
                request.beneficiary,
            )
            .map_err(|_| ResolutionError::Funding)?,
        )
        .map_err(|_| ResolutionError::Funding)?;
        if plan.native_rent_credit() != request.beneficiary
            || plan.remaining_realm_collateral() != 0
            || plan.realm_token_beneficiary().is_some()
        {
            return Err(ResolutionError::Funding.into());
        }
        ledger_remaining_native_principal = ledger_remaining_native_principal
            .checked_add(plan.remaining_native_lamports())
            .ok_or(ResolutionError::Arithmetic)?;
        if plan.ledger_can_close() {
            ledger_rent_lamports = plan.ledger_rent_lamports();
            ledger_lamport_surplus = plan.ledger_lamport_donation();
        } else if plan.ledger_rent_lamports() != 0 || plan.ledger_lamport_donation() != 0 {
            return Err(ResolutionError::Funding.into());
        }
        planned_ledger_lamports = plan.expected_post_ledger_lamports();
        ledger_can_close = plan.ledger_can_close();
    }
    if !ledger_can_close || planned_ledger_lamports != 0 {
        return Err(ResolutionError::Funding.into());
    }
    let certificate_data = certificate_account
        .try_borrow_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let terminal_kind = match source.phase() {
        SourceResolutionPhaseV1::Retired => {
            if terminal.route() == SourceResolutionRouteV1::Failure {
                ResolutionCoreReceiptKindV1::TerminalFailure
            } else {
                ResolutionCoreReceiptKindV1::TerminalSuccess
            }
        }
        _ => return Err(ResolutionError::Transition.into()),
    };
    authenticate_admitted_terminal_certificate_v2(
        program_id,
        common.source_state,
        certificate_account,
        terminal_kind,
        terminal.terminal_sequence(),
        request.source_material,
        common.market.key.to_bytes(),
        authenticated.state.identity.product_record.to_bytes(),
        authenticated.state.identity.generation,
        terminal.selector(),
        &certificate_data,
        rent,
    )?;
    let funding_set_digest = funding_set_digest(&ledger_prestate);
    let ledger_refund = common.funding_ledger.lamports();
    let source_refund = common.source_state.lamports();
    if source_refund < rent.minimum_balance(SOURCE_RESOLUTION_STATE_BYTES_V2) {
        return Err(ResolutionError::Funding.into());
    }
    if ledger_remaining_native_principal
        .checked_add(ledger_rent_lamports)
        .and_then(|value| value.checked_add(ledger_lamport_surplus))
        != Some(ledger_refund)
    {
        return Err(ResolutionError::Funding.into());
    }
    let refund_lamports = source_refund
        .checked_add(ledger_remaining_native_principal)
        .and_then(|value| value.checked_add(ledger_rent_lamports))
        .and_then(|value| value.checked_add(ledger_lamport_surplus))
        .ok_or(ResolutionError::Arithmetic)?;
    let beneficiary_lamports = beneficiary
        .lamports()
        .checked_add(refund_lamports)
        .ok_or(ResolutionError::Arithmetic)?;
    let closure = SourceClosureReceiptV3 {
        market: common.market.key.to_bytes(),
        source_state: common.source_state.key.to_bytes(),
        source_material: request.source_material,
        capability_manifest: request.capability_manifest,
        terminal_certificate: certificate_account.key.to_bytes(),
        receipt_account: closure_account.key.to_bytes(),
        beneficiary: request.beneficiary,
        source_state_digest: hash(&source_data).to_bytes(),
        terminal_certificate_digest: hash(&certificate_data).to_bytes(),
        funding_set_digest,
        generation: authenticated.state.identity.generation,
        terminal_sequence: terminal.terminal_sequence(),
        selector: terminal.selector(),
        source_refund_lamports: source_refund,
        ledger_remaining_native_principal,
        ledger_rent_lamports,
        ledger_lamport_surplus,
        refund_lamports,
        closed_at: u64::try_from(clock.unix_timestamp).map_err(|_| ResolutionError::Arithmetic)?,
    };
    let closure_bytes = closure
        .to_bytes()
        .map_err(|_| ResolutionError::OutputState)?;
    let post_digest = poststate_digest(request.action, &closure_bytes, &[], None)?;
    drop(certificate_data);
    drop(source_data);
    drop(manifest_data);
    initialize_closure_output(
        program_id,
        common.source_state,
        closure_account,
        request.receipt_sequence,
        system,
        rent,
    )?;
    write_state(closure_account, &closure_bytes)?;
    commit_refund(
        common.source_state,
        common.funding_ledger,
        beneficiary,
        beneficiary_lamports,
    )?;
    return_ack(
        program_id,
        envelope,
        authenticated.full_effect_digest,
        post_digest,
        terminal.terminal_sequence(),
        closure_sequence,
        1,
        2,
    )
}

fn authenticate_funding_entries(
    material: SourceMaterialV3,
    recovery_policy: Option<RecoveryPolicyV2>,
    manifest: CapabilityManifestV1<'_>,
    request: ResolutionRoleRequestV2,
) -> ProgramResult {
    match (material.recovery_policy(), recovery_policy) {
        (Some(recovery_policy_id), Some(recovery_policy)) => {
            if recovery_policy.attempt_count() != 1 {
                return Err(ResolutionError::SourceMaterial.into());
            }
            let recovery_allocation = recovery_policy
                .attempt(0)
                .map_err(|_| ResolutionError::SourceMaterial)?
                .funding_allocation_id()
                .to_bytes();
            for (index, expected_config) in [
                (request.recovery_entry_index, recovery_allocation),
                (
                    request.exhaustion_entry_index,
                    recovery_policy_id.to_bytes(),
                ),
                (request.failure_entry_index, request.source_material),
            ] {
                let entry = manifest
                    .entry(index)
                    .map_err(|_| ResolutionError::Funding)?;
                if entry.config_id().to_bytes() != expected_config
                    || entry.release_id().to_bytes() != RESOLUTION_CONTROLLER_RELEASE_ID_V7
                {
                    return Err(ResolutionError::Funding.into());
                }
            }
            Ok(())
        }
        // The §12.7 no-recovery material. There is no allocation identity and
        // no policy digest to pin the recovery and exhaustion entries to, so
        // the rule is structural: three pairwise-distinct Resolution-controller
        // entries, exactly one of which — the failure entry — is configured by
        // this market's own Source material. `funded::plan_funding_release`
        // admits the escrow by that same configuration comparison, so the two
        // non-material compartments can never stand in for it; they exist,
        // prepaid, until `CloseFund` refunds them.
        (None, None) => {
            if request.recovery_entry_index == request.exhaustion_entry_index
                || request.recovery_entry_index == request.failure_entry_index
                || request.exhaustion_entry_index == request.failure_entry_index
            {
                return Err(ResolutionError::Funding.into());
            }
            let mut configs = [[0_u8; 32]; 3];
            for (slot, index) in [
                request.recovery_entry_index,
                request.exhaustion_entry_index,
                request.failure_entry_index,
            ]
            .into_iter()
            .enumerate()
            {
                let entry = manifest
                    .entry(index)
                    .map_err(|_| ResolutionError::Funding)?;
                if entry.release_id().to_bytes() != RESOLUTION_CONTROLLER_RELEASE_ID_V7 {
                    return Err(ResolutionError::Funding.into());
                }
                if let Some(config) = configs.get_mut(slot) {
                    *config = entry.config_id().to_bytes();
                }
            }
            let [recovery_config, exhaustion_config, failure_config] = configs;
            if failure_config != request.source_material
                || recovery_config == request.source_material
                || exhaustion_config == request.source_material
                || recovery_config == exhaustion_config
            {
                return Err(ResolutionError::Funding.into());
            }
            Ok(())
        }
        // The caller's authentication and the material disagree about whether
        // a recovery policy exists, which is a bug in this program rather than
        // a hostile input; refuse rather than pick a side.
        _ => Err(ResolutionError::SourceMaterial.into()),
    }
}

#[inline(never)]
fn authenticate_finalized_funding_policy(
    common: CommonAccounts<'_, '_>,
    raw: Option<&AccountInfo<'_>>,
    staging: Option<&AccountInfo<'_>>,
    request: &ResolutionRoleRequestV2,
    rent: &Rent,
) -> ProgramResult {
    let material_data = common
        .source_material
        .try_borrow_data()
        .map_err(|_| ResolutionError::SourceMaterial)?;
    let material =
        SourceMaterialV3::decode(&material_data).map_err(|_| ResolutionError::SourceMaterial)?;
    let recovery_policy = authenticate_recovery_policy(common, raw, staging, material, rent)?;
    let manifest_data = common
        .capability_manifest
        .try_borrow_data()
        .map_err(|_| ResolutionError::Funding)?;
    let manifest =
        CapabilityManifestV1::decode(&manifest_data).map_err(|_| ResolutionError::Funding)?;
    authenticate_funding_entries(material, recovery_policy, manifest, *request)
}

fn authenticate_recovery_policy(
    common: CommonAccounts<'_, '_>,
    raw: Option<&AccountInfo<'_>>,
    staging: Option<&AccountInfo<'_>>,
    material: SourceMaterialV3,
    rent: &Rent,
) -> Result<Option<RecoveryPolicyV2>, ProgramError> {
    match (material.recovery_policy(), raw, staging) {
        (Some(policy_id), Some(raw), Some(staging)) => {
            let policy_data = raw
                .try_borrow_data()
                .map_err(|_| ResolutionError::FinalizedRecord)?;
            authenticate_finalized_record(
                *common.registry_program.key,
                raw,
                staging,
                rent,
                RECOVERY_POLICY_SCHEMA_ID_V2,
                policy_id.to_bytes(),
                &policy_data,
                RecordKind::RecoveryPolicyV2,
            )?;
            RecoveryPolicyV2::decode(&policy_data)
                .map(Some)
                .map_err(|_| ResolutionError::SourceMaterial.into())
        }
        // The no-recovery material has no policy record and therefore no
        // frame positions carrying one: the short frame IS the statement
        // that none exists, and the authenticated material is what makes
        // that statement checkable rather than a caller's choice.
        (None, None, None) => Ok(None),
        // A frame width that disagrees with the authenticated material —
        // policy positions without a policy, or a policy with nowhere to
        // present it — is refused rather than reconciled.
        _ => Err(ResolutionError::AccountFrame.into()),
    }
}

fn copy_ledger_bytes(
    account: &AccountInfo<'_>,
) -> Result<[u8; RESOLUTION_FUNDING_LEDGER_BYTES], ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| ResolutionError::Funding)?;
    let mut output = [0_u8; RESOLUTION_FUNDING_LEDGER_BYTES];
    if data.len() != output.len() {
        return Err(ResolutionError::Funding.into());
    }
    output.copy_from_slice(&data);
    Ok(output)
}

#[allow(clippy::too_many_arguments)]
fn authenticate_live_ledger(
    program_id: &Pubkey,
    common: CommonAccounts<'_, '_>,
    generation: u64,
    manifest_id: CapabilityContentId,
    manifest: CapabilityManifestV1<'_>,
    request: ResolutionRoleRequestV2,
    expected_status: FundingLedgerStatusV2,
    bytes: &[u8],
    observed_lamports: u64,
    rent: &Rent,
    admit_donations: bool,
) -> ProgramResult {
    if common.funding_ledger.owner != program_id
        || common.funding_ledger.executable
        || common.funding_ledger.data_len() != RESOLUTION_FUNDING_LEDGER_BYTES
    {
        return Err(ResolutionError::Funding.into());
    }
    authenticate_ledger_value(
        program_id,
        common.market,
        common.funding_ledger,
        generation,
        manifest_id,
        manifest,
        request,
        expected_status,
        bytes,
        observed_lamports,
        rent,
        admit_donations,
    )
}

#[allow(clippy::too_many_arguments)]
fn authenticate_ledger_value(
    program_id: &Pubkey,
    market: &AccountInfo<'_>,
    funding_ledger: &AccountInfo<'_>,
    generation: u64,
    manifest_id: CapabilityContentId,
    manifest: CapabilityManifestV1<'_>,
    request: ResolutionRoleRequestV2,
    expected_status: FundingLedgerStatusV2,
    bytes: &[u8],
    observed_lamports: u64,
    rent: &Rent,
    admit_donations: bool,
) -> ProgramResult {
    if bytes.len() != funding_ledger_bytes_v2(3).map_err(|_| ResolutionError::Funding)? {
        return Err(ResolutionError::Funding.into());
    }
    let ledger = FundingLedgerV2::decode(bytes).map_err(|_| ResolutionError::Funding)?;
    let expected_mask = request
        .funding_entry_mask()
        .map_err(|_| ResolutionError::Funding)?;
    if ledger.selected_mask() != expected_mask || ledger.slot_count() != 3 {
        return Err(ResolutionError::Funding.into());
    }
    let authenticated = ledger
        .authenticate(manifest_id, manifest)
        .map_err(|_| ResolutionError::Funding)?;
    for entry_index in [
        request.recovery_entry_index,
        request.exhaustion_entry_index,
        request.failure_entry_index,
    ] {
        let slot = authenticated
            .slot(entry_index)
            .map_err(|_| ResolutionError::Funding)?;
        if slot.status() != expected_status
            || slot.remaining().realm_collateral_total() != 0
            || slot.released().realm_collateral_total() != 0
        {
            return Err(ResolutionError::Funding.into());
        }
    }
    authenticated
        .validate_native_custody(
            observed_lamports,
            rent.minimum_balance(RESOLUTION_FUNDING_LEDGER_BYTES),
            admit_donations,
        )
        .map_err(|_| ResolutionError::Funding)?;
    let derivation = CapabilityFundingLedgerDerivationV2::new(
        program_id.to_bytes(),
        market.key.to_bytes(),
        generation,
        manifest_id,
        ledger,
    )
    .map_err(|_| ResolutionError::Funding)?;
    if Pubkey::find_program_address(&derivation.seed_components(), program_id).0
        != *funding_ledger.key
    {
        return Err(ResolutionError::Funding.into());
    }
    Ok(())
}

fn require_revisions(
    envelope: &CoreEffectEnvelopeV1,
    resource_a: u64,
    resource_b: u64,
) -> ProgramResult {
    if envelope.expected_resource_a_revision() != resource_a
        || envelope.expected_resource_b_revision() != resource_b
    {
        return Err(ResolutionError::Transition.into());
    }
    Ok(())
}

fn action_byte(action: ResolutionCoreActionV1) -> u8 {
    match action {
        ResolutionCoreActionV1::CreateFund => CoreEffectActionV1::CreateFund as u8,
        ResolutionCoreActionV1::VerifyFundReady => CoreEffectActionV1::VerifyFundReady as u8,
        ResolutionCoreActionV1::AdmitTerminal => CoreEffectActionV1::AdmitTerminal as u8,
        ResolutionCoreActionV1::CloseFund => CoreEffectActionV1::CloseFund as u8,
    }
}

#[inline(never)]
fn poststate_digest(
    action: ResolutionCoreActionV1,
    source_or_closure: &[u8],
    funding_ledger: &[u8],
    certificate: Option<&[u8]>,
) -> Result<Identity, ProgramError> {
    let action = [action_byte(action)];
    Identity::new(
        hashv(&[
            RESOLUTION_POSTSTATE_DIGEST_DOMAIN_V2,
            &action,
            source_or_closure,
            funding_ledger,
            certificate.unwrap_or(&[]),
        ])
        .to_bytes(),
    )
    .map_err(|_| ResolutionError::OutputState.into())
}

#[inline(never)]
fn funding_set_digest(funding_ledger: &[u8]) -> [u8; 32] {
    hashv(&[SOURCE_FUNDING_SET_DIGEST_DOMAIN_V2, funding_ledger]).to_bytes()
}

fn require_prepaid_output(account: &AccountInfo<'_>, minimum_lamports: u64) -> ProgramResult {
    if account.owner != &system_program::ID
        || account.data_len() != 0
        || account.executable
        || !account.is_writable
        || account.lamports() < minimum_lamports
    {
        return Err(ResolutionError::OutputState.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn initialize_source_output<'info>(
    program_id: &Pubkey,
    market: &AccountInfo<'info>,
    output: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    generation: u64,
    bump: u8,
    rent: &Rent,
) -> ProgramResult {
    let generation_seed = generation.to_le_bytes();
    let bump_seed = [bump];
    let signer: [&[u8]; 4] = [
        dclutch_source_contract::SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
        market.key.as_ref(),
        &generation_seed,
        &bump_seed,
    ];
    let space =
        u64::try_from(SOURCE_RESOLUTION_STATE_BYTES_V2).map_err(|_| ResolutionError::Arithmetic)?;
    invoke_signed(
        &allocate(output.key, space),
        &[output.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| ResolutionError::OutputState)?;
    invoke_signed(
        &assign(output.key, program_id),
        &[output.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| ResolutionError::OutputState)?;
    if output.owner != program_id
        || output.executable
        || output.data_len() != SOURCE_RESOLUTION_STATE_BYTES_V2
        || output.lamports() < rent.minimum_balance(SOURCE_RESOLUTION_STATE_BYTES_V2)
    {
        return Err(ResolutionError::OutputState.into());
    }
    Ok(())
}

fn write_state(account: &AccountInfo<'_>, bytes: &[u8]) -> ProgramResult {
    let mut output = account
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    if output.len() != bytes.len() || output.iter().any(|byte| *byte != 0) {
        return Err(ResolutionError::OutputState.into());
    }
    output.copy_from_slice(bytes);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn return_ack(
    program_id: &Pubkey,
    envelope: &CoreEffectEnvelopeV1,
    full_effect_digest: Identity,
    post_digest: Identity,
    pre_a: u64,
    post_a: u64,
    pre_b: u64,
    post_b: u64,
) -> ProgramResult {
    let encoded = build_ack(
        program_id,
        envelope,
        full_effect_digest,
        post_digest,
        pre_a,
        post_a,
        pre_b,
        post_b,
    )?;
    set_return_data(&encoded);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_ack(
    program_id: &Pubkey,
    envelope: &CoreEffectEnvelopeV1,
    full_effect_digest: Identity,
    post_digest: Identity,
    pre_a: u64,
    post_a: u64,
    pre_b: u64,
    post_b: u64,
) -> Result<[u8; CORE_EFFECT_ACK_BYTES_V1], ProgramError> {
    let role_program =
        Identity::new(program_id.to_bytes()).map_err(|_| ResolutionError::ResolutionRelease)?;
    let ack = CoreEffectAckV1::new(
        envelope.action(),
        Role::Resolution,
        role_program,
        envelope.release_set(),
        envelope.market(),
        envelope.context(),
        full_effect_digest,
        post_digest,
        pre_a,
        post_a,
        pre_b,
        post_b,
    )
    .map_err(|_| ResolutionError::Transition)?;
    let encoded = ack.encode().map_err(|_| ResolutionError::Transition)?;
    if encoded.len() != CORE_EFFECT_ACK_BYTES_V1 {
        return Err(ResolutionError::Transition.into());
    }
    Ok(encoded)
}

fn commit_activated_ledger(
    ledger: &AccountInfo<'_>,
    ledger_bytes: &[u8; RESOLUTION_FUNDING_LEDGER_BYTES],
    ledger_lamports_after: u64,
    beneficiary: &AccountInfo<'_>,
    beneficiary_lamports_after: u64,
) -> ProgramResult {
    let mut ledger_data = ledger
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut ledger_lamports = ledger
        .try_borrow_mut_lamports()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut beneficiary_lamports = beneficiary
        .try_borrow_mut_lamports()
        .map_err(|_| ResolutionError::OutputState)?;
    if ledger_data.len() != RESOLUTION_FUNDING_LEDGER_BYTES {
        return Err(ResolutionError::OutputState.into());
    }
    ledger_data.copy_from_slice(ledger_bytes);
    **ledger_lamports = ledger_lamports_after;
    **beneficiary_lamports = beneficiary_lamports_after;
    Ok(())
}

fn authenticate_terminal_source(
    program_id: &Pubkey,
    common: CommonAccounts<'_, '_>,
    request: &ResolutionRoleRequestV2,
    state: &CoreState,
    bytes: &[u8],
) -> Result<SourceResolutionStateV2, ProgramError> {
    let source =
        SourceResolutionStateV2::decode(bytes).map_err(|_| ResolutionError::OutputState)?;
    authenticate_state_account_v2(program_id, common.source_state, source)?;
    if !matches!(
        source.phase(),
        SourceResolutionPhaseV1::Resolved | SourceResolutionPhaseV1::FailureCommitted
    ) || source.market() != common.market.key.to_bytes()
        || source.generation() != state.identity.generation
        || source.material_id().to_bytes() != request.source_material
    {
        return Err(ResolutionError::Transition.into());
    }
    Ok(source)
}

fn authenticate_admit_product_runtime(
    common: CommonAccounts<'_, '_>,
    state: &CoreState,
    material: SourceMaterialV3,
    accounts: &[AccountInfo<'_>],
    rent: &Rent,
) -> Result<AuthenticatedProductRuntimeV2, ProgramError> {
    let product = FinalizedRecordFrameV2 {
        raw: accounts.get(16).ok_or(ResolutionError::AccountFrame)?,
        staging: accounts.get(17).ok_or(ResolutionError::AccountFrame)?,
    };
    let result_domain = FinalizedRecordFrameV2 {
        raw: accounts.get(18).ok_or(ResolutionError::AccountFrame)?,
        staging: accounts.get(19).ok_or(ResolutionError::AccountFrame)?,
    };
    let portfolio = FinalizedRecordFrameV2 {
        raw: accounts.get(20).ok_or(ResolutionError::AccountFrame)?,
        staging: accounts.get(21).ok_or(ResolutionError::AccountFrame)?,
    };
    let expected_product = ProductContentId::new(material.product_record_digest().to_bytes())
        .map_err(|_| ResolutionError::SourceMaterial)?;
    let runtime = authenticate_product_runtime_v2(
        common.registry_program.key,
        rent,
        expected_product,
        ProductRuntimeFrameV2 {
            product,
            result_domain,
            portfolio,
        },
    )
    .map_err(|_| ResolutionError::ProductDomain)?;
    if runtime.product_record.content_digest.to_bytes() != state.identity.product_record.to_bytes()
        || runtime.product_id.to_bytes() != state.identity.product_id.to_bytes()
    {
        return Err(ResolutionError::ProductDomain.into());
    }
    Ok(runtime)
}

fn authenticate_state_account_v2(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    state: SourceResolutionStateV2,
) -> ProgramResult {
    if account.owner != program_id
        || account.data_len() != SOURCE_RESOLUTION_STATE_BYTES_V2
        || account.executable
    {
        return Err(ResolutionError::OutputState.into());
    }
    let seeds = state.pda_seeds();
    let bump = [seeds.bump()];
    let expected = Pubkey::create_program_address(
        &[
            seeds.domain(),
            &seeds.market(),
            &seeds.generation_le(),
            &bump,
        ],
        program_id,
    )
    .map_err(|_| ResolutionError::OutputState)?;
    if account.key != &expected {
        return Err(ResolutionError::OutputState.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn authenticate_terminal_certificate_v2(
    program_id: &Pubkey,
    source_state: &AccountInfo<'_>,
    account: &AccountInfo<'_>,
    receipt_kind: ResolutionCoreReceiptKindV1,
    sequence: u64,
    source_material: [u8; 32],
    market: [u8; 32],
    product_record: [u8; 32],
    generation: u64,
    selector: u32,
    outcome_count: u32,
    bytes: &[u8],
    rent: &Rent,
) -> ProgramResult {
    let (expected_kind, kind_tag) = match receipt_kind {
        ResolutionCoreReceiptKindV1::TerminalSuccess => {
            (ResolutionCertificateKindV2::ResolutionSuccess, 1_u8)
        }
        ResolutionCoreReceiptKindV1::TerminalFailure => {
            (ResolutionCertificateKindV2::ResolutionFailure, 4_u8)
        }
        ResolutionCoreReceiptKindV1::None | ResolutionCoreReceiptKindV1::Closure => {
            return Err(ResolutionError::Transition.into());
        }
    };
    if account.owner != program_id
        || account.executable
        || account.data_len() != RESOLUTION_CERTIFICATE_BYTES_V2
        || account.lamports() < rent.minimum_balance(RESOLUTION_CERTIFICATE_BYTES_V2)
    {
        return Err(ResolutionError::OutputState.into());
    }
    let certificate =
        ResolutionCertificateV2::decode(bytes).map_err(|_| ResolutionError::OutputState)?;
    if certificate.kind != expected_kind
        || certificate.market != market
        || certificate.source_material != source_material
        || certificate.product_record_digest != product_record
        || certificate.receipt_account != account.key.to_bytes()
        || certificate.generation != generation
        || certificate.selector != selector
    {
        return Err(ResolutionError::Transition.into());
    }
    certificate
        .validate_terminal_product(product_record, outcome_count)
        .map_err(|_| ResolutionError::ProductDomain)?;
    let kind_seed = [kind_tag];
    let sequence_seed = sequence.to_le_bytes();
    let expected = Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            source_state.key.as_ref(),
            &kind_seed,
            &sequence_seed,
        ],
        program_id,
    )
    .0;
    if account.key != &expected {
        return Err(ResolutionError::OutputState.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_admitted_terminal_certificate_v2(
    program_id: &Pubkey,
    source_state: &AccountInfo<'_>,
    account: &AccountInfo<'_>,
    receipt_kind: ResolutionCoreReceiptKindV1,
    sequence: u64,
    source_material: [u8; 32],
    market: [u8; 32],
    product_record: [u8; 32],
    generation: u64,
    selector: u32,
    bytes: &[u8],
    rent: &Rent,
) -> ProgramResult {
    let (expected_kind, kind_tag) = match receipt_kind {
        ResolutionCoreReceiptKindV1::TerminalSuccess => {
            (ResolutionCertificateKindV2::ResolutionSuccess, 1_u8)
        }
        ResolutionCoreReceiptKindV1::TerminalFailure => {
            (ResolutionCertificateKindV2::ResolutionFailure, 4_u8)
        }
        ResolutionCoreReceiptKindV1::None | ResolutionCoreReceiptKindV1::Closure => {
            return Err(ResolutionError::Transition.into());
        }
    };
    if account.owner != program_id
        || account.executable
        || account.data_len() != RESOLUTION_CERTIFICATE_BYTES_V2
        || account.lamports() < rent.minimum_balance(RESOLUTION_CERTIFICATE_BYTES_V2)
    {
        return Err(ResolutionError::OutputState.into());
    }
    let certificate =
        ResolutionCertificateV2::decode(bytes).map_err(|_| ResolutionError::OutputState)?;
    if certificate.kind != expected_kind
        || certificate.market != market
        || certificate.source_material != source_material
        || certificate.receipt_account != account.key.to_bytes()
        || certificate.generation != generation
    {
        return Err(ResolutionError::Transition.into());
    }
    certificate
        .validate_admitted_terminal(product_record, selector)
        .map_err(|_| ResolutionError::ProductDomain)?;
    let kind_seed = [kind_tag];
    let sequence_seed = sequence.to_le_bytes();
    let expected = Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            source_state.key.as_ref(),
            &kind_seed,
            &sequence_seed,
        ],
        program_id,
    )
    .0;
    if account.key != &expected {
        return Err(ResolutionError::OutputState.into());
    }
    Ok(())
}

fn initialize_closure_output<'info>(
    program_id: &Pubkey,
    source_state: &AccountInfo<'info>,
    output: &AccountInfo<'info>,
    sequence: u64,
    system: &AccountInfo<'info>,
    rent: &Rent,
) -> ProgramResult {
    let sequence_seed = sequence.to_le_bytes();
    let (expected, bump) = Pubkey::find_program_address(
        &[
            SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V3,
            source_state.key.as_ref(),
            &sequence_seed,
        ],
        program_id,
    );
    if output.key != &expected {
        return Err(ResolutionError::OutputState.into());
    }
    require_prepaid_output(
        output,
        rent.minimum_balance(SOURCE_CLOSURE_RECEIPT_BYTES_V3),
    )?;
    let bump_seed = [bump];
    let signer: [&[u8]; 4] = [
        SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V3,
        source_state.key.as_ref(),
        &sequence_seed,
        &bump_seed,
    ];
    let space =
        u64::try_from(SOURCE_CLOSURE_RECEIPT_BYTES_V3).map_err(|_| ResolutionError::Arithmetic)?;
    invoke_signed(
        &allocate(output.key, space),
        &[output.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| ResolutionError::OutputState)?;
    invoke_signed(
        &assign(output.key, program_id),
        &[output.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| ResolutionError::OutputState)?;
    if output.owner != program_id
        || output.executable
        || output.data_len() != SOURCE_CLOSURE_RECEIPT_BYTES_V3
        || output.lamports() < rent.minimum_balance(SOURCE_CLOSURE_RECEIPT_BYTES_V3)
    {
        return Err(ResolutionError::OutputState.into());
    }
    Ok(())
}

fn commit_refund(
    source: &AccountInfo<'_>,
    funding_ledger: &AccountInfo<'_>,
    beneficiary: &AccountInfo<'_>,
    beneficiary_lamports_after: u64,
) -> ProgramResult {
    let mut source_data = source
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut ledger_data = funding_ledger
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut source_lamports = source
        .try_borrow_mut_lamports()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut ledger_lamports = funding_ledger
        .try_borrow_mut_lamports()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut beneficiary_lamports = beneficiary
        .try_borrow_mut_lamports()
        .map_err(|_| ResolutionError::OutputState)?;
    if source_data.len() != SOURCE_RESOLUTION_STATE_BYTES_V2
        || ledger_data.len() != RESOLUTION_FUNDING_LEDGER_BYTES
    {
        return Err(ResolutionError::OutputState.into());
    }
    source_data.fill(0);
    ledger_data.fill(0);
    **source_lamports = 0;
    **ledger_lamports = 0;
    **beneficiary_lamports = beneficiary_lamports_after;
    Ok(())
}

fn next<'a, 'info>(
    iterator: &mut core::slice::Iter<'a, AccountInfo<'info>>,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    next_account_info(iterator).map_err(|_| ResolutionError::AccountFrame.into())
}

#[cfg(test)]
mod tests {
    use dclutch_capability_contract::{
        ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityManifestV1,
        ContentId as CapabilityContentId, FundingAmountsV1, FundingQuoteV1, MANIFEST_HEADER_BYTES,
        MAX_DEPENDENCIES_PER_CAPABILITY,
    };
    use dclutch_market_core_codec::{
        CAPABILITY_FUNDING_HEADER_BYTES_V2, CapabilityFundingHeaderV2, CoreEffectAckV1,
        CoreEffectActionV1, CoreEffectEnvelopeV1, Identity, Role,
    };
    use dclutch_resolution_codec::{
        RESOLUTION_CONTROLLER_RELEASE_ID_V6, RESOLUTION_CONTROLLER_RELEASE_ID_V7,
        RESOLUTION_CORE_ROLE_REQUEST_BYTES_V2, RESOLUTION_POSTSTATE_DIGEST_DOMAIN_V2,
        ResolutionCoreActionV1, ResolutionCoreReceiptKindV1, ResolutionRoleRequestV2,
    };
    use dclutch_source_contract::{
        ContentId as SourceContentId, RecoveryAttemptV2, RecoveryPolicyV2, SourceMaterialV3,
    };
    use solana_program::{
        hash::{hash, hashv},
        pubkey::Pubkey,
    };

    use super::{
        ADMIT_TERMINAL_ACCOUNT_COUNT, CLOSE_FUND_ACCOUNT_COUNT, CORE_EFFECT_INSTRUCTION_BYTES,
        CREATE_FUND_ACCOUNT_COUNT, VERIFY_FUND_ACCOUNT_COUNT, action_byte, authenticate_action,
        authenticate_funding_entries, authenticate_funding_header, build_ack, is_core_effect,
        poststate_digest, require_revisions,
    };
    use crate::ResolutionError;

    fn identity(byte: u8) -> Identity {
        Identity::new([byte; 32]).expect("nonzero identity")
    }

    fn capability_id(byte: u8) -> CapabilityContentId {
        CapabilityContentId::new([byte; 32]).expect("nonzero capability identity")
    }

    fn source_id(byte: u8) -> SourceContentId {
        SourceContentId::new([byte; 32]).expect("nonzero Source identity")
    }

    fn request(action: ResolutionCoreActionV1) -> ResolutionRoleRequestV2 {
        let (receipt_kind, receipt, beneficiary, receipt_sequence) = match action {
            ResolutionCoreActionV1::CreateFund | ResolutionCoreActionV1::VerifyFundReady => {
                (ResolutionCoreReceiptKindV1::None, [0; 32], [8; 32], 0)
            }
            ResolutionCoreActionV1::AdmitTerminal => (
                ResolutionCoreReceiptKindV1::TerminalSuccess,
                [7; 32],
                [0; 32],
                3,
            ),
            ResolutionCoreActionV1::CloseFund => {
                (ResolutionCoreReceiptKindV1::Closure, [7; 32], [8; 32], 4)
            }
        };
        ResolutionRoleRequestV2 {
            action,
            receipt_kind,
            source_state: [1; 32],
            source_material: [2; 32],
            capability_manifest: [3; 32],
            funding_ledger: [4; 32],
            receipt,
            beneficiary,
            recovery_entry_index: 0,
            exhaustion_entry_index: 1,
            failure_entry_index: 2,
            receipt_sequence,
        }
    }

    fn envelope(
        action: ResolutionCoreActionV1,
        expected_a: u64,
        expected_b: u64,
    ) -> CoreEffectEnvelopeV1 {
        let role_bytes = role_bytes(action);
        let core_action = match action {
            ResolutionCoreActionV1::CreateFund => {
                dclutch_market_core_codec::CoreEffectActionV1::CreateFund
            }
            ResolutionCoreActionV1::VerifyFundReady => {
                dclutch_market_core_codec::CoreEffectActionV1::VerifyFundReady
            }
            ResolutionCoreActionV1::AdmitTerminal => {
                dclutch_market_core_codec::CoreEffectActionV1::AdmitTerminal
            }
            ResolutionCoreActionV1::CloseFund => {
                dclutch_market_core_codec::CoreEffectActionV1::CloseFund
            }
        };
        CoreEffectEnvelopeV1::new(
            core_action,
            Role::Resolution,
            identity(9),
            identity(10),
            identity(11),
            identity(12),
            identity(1),
            identity(13),
            Identity::new(hash(&role_bytes).to_bytes()).expect("request digest"),
            1,
            expected_a,
            expected_b,
            u32::try_from(role_bytes.len()).expect("fixed request width"),
        )
        .expect("envelope")
    }

    fn role_bytes(
        action: ResolutionCoreActionV1,
    ) -> [u8; CAPABILITY_FUNDING_HEADER_BYTES_V2 + RESOLUTION_CORE_ROLE_REQUEST_BYTES_V2] {
        let request = request(action);
        let mut output =
            [0_u8; CAPABILITY_FUNDING_HEADER_BYTES_V2 + RESOLUTION_CORE_ROLE_REQUEST_BYTES_V2];
        output
            .get_mut(..CAPABILITY_FUNDING_HEADER_BYTES_V2)
            .expect("funding prefix")
            .copy_from_slice(
                &CapabilityFundingHeaderV2::new(1, 3, 0b111)
                    .expect("three funds")
                    .encode(),
            );
        output
            .get_mut(CAPABILITY_FUNDING_HEADER_BYTES_V2..)
            .expect("request tail")
            .copy_from_slice(&request.to_bytes().expect("request encodes"));
        output
    }

    #[test]
    fn exact_core_effect_dispatch_and_action_partition() {
        assert_eq!(CREATE_FUND_ACCOUNT_COUNT, 18);
        assert_eq!(VERIFY_FUND_ACCOUNT_COUNT, 19);
        assert_eq!(ADMIT_TERMINAL_ACCOUNT_COUNT, 22);
        assert_eq!(CLOSE_FUND_ACCOUNT_COUNT, 22);
        for action in [
            ResolutionCoreActionV1::CreateFund,
            ResolutionCoreActionV1::VerifyFundReady,
            ResolutionCoreActionV1::AdmitTerminal,
            ResolutionCoreActionV1::CloseFund,
        ] {
            let envelope = envelope(action, 0, 0);
            authenticate_action(envelope, request(action)).expect("matching action");
            assert_eq!(action_byte(action), envelope.action() as u8);
            let role_bytes = role_bytes(action);
            let funding_header = CapabilityFundingHeaderV2::decode(
                role_bytes
                    .get(..CAPABILITY_FUNDING_HEADER_BYTES_V2)
                    .expect("funding header"),
            )
            .expect("composite role bytes");
            authenticate_funding_header(funding_header, request(action))
                .expect("exact funding count");
            let request_tail = role_bytes
                .get(CAPABILITY_FUNDING_HEADER_BYTES_V2..)
                .expect("request tail");
            assert_eq!(request_tail, request(action).to_bytes().expect("request"));
            let mut instruction = [0_u8; CORE_EFFECT_INSTRUCTION_BYTES];
            let envelope_bytes = envelope.encode().expect("envelope encodes");
            instruction
                .get_mut(..envelope_bytes.len())
                .expect("envelope prefix")
                .copy_from_slice(&envelope_bytes);
            instruction
                .get_mut(envelope_bytes.len()..)
                .expect("request tail")
                .copy_from_slice(&role_bytes);
            assert!(is_core_effect(&instruction));
            let short = instruction
                .get(..instruction.len().saturating_sub(1))
                .expect("short instruction");
            assert!(!is_core_effect(short));
        }
        assert_eq!(
            authenticate_action(
                envelope(ResolutionCoreActionV1::CreateFund, 0, 0),
                request(ResolutionCoreActionV1::VerifyFundReady),
            ),
            Err(ResolutionError::Instruction.into())
        );

        let exact = role_bytes(ResolutionCoreActionV1::CreateFund);
        let wrong_count = CapabilityFundingHeaderV2::new(1, 2, 0b11).expect("bounded count");
        assert_eq!(
            authenticate_funding_header(wrong_count, request(ResolutionCoreActionV1::CreateFund),),
            Err(ResolutionError::Instruction.into())
        );

        let mut hostile_header = exact;
        hostile_header[0] ^= 1;
        assert!(
            CapabilityFundingHeaderV2::decode(
                hostile_header
                    .get(..CAPABILITY_FUNDING_HEADER_BYTES_V2)
                    .expect("funding header"),
            )
            .is_err()
        );

        let envelope = envelope(ResolutionCoreActionV1::CreateFund, 0, 0);
        let exact_digest = Identity::new(hash(&exact).to_bytes()).expect("composite digest");
        envelope
            .validate_role_request(exact.len(), exact_digest)
            .expect("full composite is bound");
        let tail = exact
            .get(CAPABILITY_FUNDING_HEADER_BYTES_V2..)
            .expect("child request tail");
        let tail_digest = Identity::new(hash(tail).to_bytes()).expect("tail digest");
        assert!(
            envelope
                .validate_role_request(tail.len(), tail_digest)
                .is_err()
        );
    }

    #[test]
    fn finalized_v3_material_is_the_only_three_funding_config_authority() {
        let material_id = source_id(2);
        let recovery_policy_id = source_id(15);
        let recovery_allocation = source_id(14);
        let material = SourceMaterialV3::explicitly_unbounded(
            source_id(20),
            source_id(21),
            source_id(22),
            source_id(23),
            Some(recovery_policy_id),
            source_id(24),
        );
        let policy = RecoveryPolicyV2::new(
            source_id(25),
            [
                Some(
                    RecoveryAttemptV2::new(source_id(26), source_id(27), 100, recovery_allocation)
                        .expect("attempt"),
                ),
                None,
                None,
                None,
            ],
            1,
        )
        .expect("policy");
        let quote = FundingQuoteV1::new(FundingAmountsV1::default(), None).expect("zero quote");
        let configs = [
            recovery_allocation.to_bytes(),
            recovery_policy_id.to_bytes(),
            material_id.to_bytes(),
        ];
        let mut entries = [CapabilityEntryV1::new(
            capability_id(30),
            capability_id(RESOLUTION_CONTROLLER_RELEASE_ID_V7[0]),
            capability_id(31),
            capability_id(32),
            capability_id(33),
            capability_id(34),
            ActivationPolicy::RequiredAtFounding,
            0,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            quote,
        )
        .expect("placeholder"); 3];
        for (index, (entry, config)) in entries.iter_mut().zip(configs).enumerate() {
            *entry = CapabilityEntryV1::new(
                capability_id(u8::try_from(40 + index).expect("bounded")),
                CapabilityContentId::new(RESOLUTION_CONTROLLER_RELEASE_ID_V7).expect("release"),
                CapabilityContentId::new(config).expect("config"),
                capability_id(50),
                capability_id(51),
                capability_id(52),
                ActivationPolicy::RequiredAtFounding,
                0,
                0,
                [0; MAX_DEPENDENCIES_PER_CAPABILITY],
                quote,
            )
            .expect("entry");
        }
        let mut bytes = [0_u8; MANIFEST_HEADER_BYTES + 3 * CAPABILITY_ENTRY_BYTES];
        let manifest = CapabilityManifestV1::encode_into(&entries, &mut bytes).expect("manifest");
        let exact = request(ResolutionCoreActionV1::CreateFund);
        authenticate_funding_entries(material, Some(policy), manifest, exact).expect("exact join");

        let mut v6_entries = entries;
        for (index, (entry, config)) in v6_entries.iter_mut().zip(configs).enumerate() {
            *entry = CapabilityEntryV1::new(
                capability_id(u8::try_from(40 + index).expect("bounded")),
                CapabilityContentId::new(RESOLUTION_CONTROLLER_RELEASE_ID_V6)
                    .expect("legacy release"),
                CapabilityContentId::new(config).expect("config"),
                capability_id(50),
                capability_id(51),
                capability_id(52),
                ActivationPolicy::RequiredAtFounding,
                0,
                0,
                [0; MAX_DEPENDENCIES_PER_CAPABILITY],
                quote,
            )
            .expect("legacy entry");
        }
        let mut v6_bytes = [0_u8; MANIFEST_HEADER_BYTES + 3 * CAPABILITY_ENTRY_BYTES];
        let v6_manifest =
            CapabilityManifestV1::encode_into(&v6_entries, &mut v6_bytes).expect("V6 manifest");
        assert_eq!(
            authenticate_funding_entries(material, Some(policy), v6_manifest, exact),
            Err(ResolutionError::Funding.into()),
            "the V7 program must not activate a V6 controller ledger"
        );

        let substituted_policy = RecoveryPolicyV2::new(
            source_id(25),
            [
                Some(
                    RecoveryAttemptV2::new(source_id(26), source_id(27), 100, source_id(99))
                        .expect("attempt"),
                ),
                None,
                None,
                None,
            ],
            1,
        )
        .expect("policy");
        assert_eq!(
            authenticate_funding_entries(material, Some(substituted_policy), manifest, exact),
            Err(ResolutionError::Funding.into())
        );
        let no_recovery = SourceMaterialV3::explicitly_unbounded(
            source_id(20),
            source_id(21),
            source_id(22),
            source_id(23),
            None,
            source_id(24),
        );
        // The two ways the caller's authentication and the material can
        // disagree about whether a recovery policy exists. Both are this
        // program contradicting itself rather than a hostile input, and both
        // must refuse rather than pick a side. `e5b69230` widened this
        // parameter to `Option` for the §12.7 no-recovery material and left
        // these three call sites uncompiled; the second disagreement had no
        // case at all.
        assert_eq!(
            authenticate_funding_entries(no_recovery, Some(policy), manifest, exact),
            Err(ResolutionError::SourceMaterial.into())
        );
        assert_eq!(
            authenticate_funding_entries(material, None, manifest, exact),
            Err(ResolutionError::SourceMaterial.into())
        );
    }

    #[test]
    fn revisions_and_poststate_digest_refuse_substitution() {
        let envelope = envelope(ResolutionCoreActionV1::AdmitTerminal, 3, 1);
        require_revisions(&envelope, 3, 1).expect("exact revisions");
        assert_eq!(
            require_revisions(&envelope, 2, 1),
            Err(ResolutionError::Transition.into())
        );
        let exact = poststate_digest(
            ResolutionCoreActionV1::AdmitTerminal,
            &[1],
            &[2],
            Some(&[5]),
        )
        .expect("digest");
        let reordered = poststate_digest(
            ResolutionCoreActionV1::AdmitTerminal,
            &[1],
            &[3],
            Some(&[5]),
        )
        .expect("digest");
        let no_certificate =
            poststate_digest(ResolutionCoreActionV1::AdmitTerminal, &[1], &[2], None)
                .expect("digest");
        assert_ne!(exact, reordered);
        assert_ne!(exact, no_certificate);
        let action = [CoreEffectActionV1::AdmitTerminal as u8];
        let core_derived = Identity::new(
            hashv(&[
                RESOLUTION_POSTSTATE_DIGEST_DOMAIN_V2,
                &action,
                &[1],
                &[2],
                &[5],
            ])
            .to_bytes(),
        )
        .expect("Core-derived poststate digest");
        assert_eq!(exact, core_derived);
    }

    #[test]
    fn acknowledgement_is_the_only_return_wire_and_binds_effect() {
        let program_id = Pubkey::new_from_array([21; 32]);
        let envelope = envelope(ResolutionCoreActionV1::VerifyFundReady, 0, 0);
        let effect_digest = identity(22);
        let post_digest = identity(23);
        let bytes = build_ack(
            &program_id,
            &envelope,
            effect_digest,
            post_digest,
            0,
            0,
            0,
            1,
        )
        .expect("ack encodes");
        let ack = CoreEffectAckV1::decode(&bytes).expect("one exact ack");
        assert_eq!(ack.post_resource_digest(), post_digest);
        ack.validate_for(
            envelope,
            Identity::new(program_id.to_bytes()).expect("program identity"),
            effect_digest,
        )
        .expect("effect binding");
        assert_eq!(ack.post_resource_b_revision(), 1);
    }
}
