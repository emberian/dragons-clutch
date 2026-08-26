//! Resolution-role child composition through the canonical Core authority.

use dclutch_capability_contract::{
    CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityFundingDerivationV1, CapabilityManifestV1,
    ContentId as CapabilityContentId, FUNDING_STATE_BYTES, FundingCustodyObservationV1,
    FundingStateV1, FundingStatus,
};
use dclutch_market_core_codec::{
    Action, CapabilityFundingHeaderV1, ChildEffectObservation, CoreEffectAckV1, CoreEffectActionV1,
    CoreEffectEnvelopeV1, CoreState, Product, Readiness, Request, Role, TerminalReceipt,
    admit_terminal, verify_readiness,
};
use dclutch_product_contract::{
    product::{
        INSTANCE_BYTES, InstanceV1, PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1,
        PRODUCT_TERMS_SCHEMA_RELEASE_ID_V1, TERMS_BYTES, TermsV1,
    },
    result_domain::FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1,
};
use dclutch_resolution_codec::{
    RESOLUTION_CERTIFICATE_BYTES, RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
    RESOLUTION_CORE_ROLE_REQUEST_BYTES, RESOLUTION_POSTSTATE_DIGEST_DOMAIN_V1,
    ResolutionCertificateKindV1, ResolutionCertificateV1, ResolutionCoreActionV1,
    ResolutionCoreReceiptKindV1, ResolutionRoleRequestV1, SOURCE_CLOSURE_RECEIPT_BYTES,
    SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V1, SOURCE_FUNDING_SET_DIGEST_DOMAIN_V1,
    SourceClosureReceiptV1,
};
use dclutch_source_contract::{
    SOURCE_MATERIAL_BYTES, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1, SOURCE_RESOLUTION_STATE_BYTES,
    SourceMaterialViewV1, SourceResolutionPhaseV1, SourceResolutionStateV1,
};
use solana_program::{
    account_info::AccountInfo, hash::hashv, program_error::ProgramError, pubkey::Pubkey,
    rent::Rent, sysvar::SysvarSerialize,
};
use solana_sdk_ids::{system_program, sysvar};

use crate::{
    CoreSbfError,
    fixed_role::{
        FixedRoleAccountsV1, authenticate_fixed_role, authenticate_fixed_role_ack,
        invoke_fixed_role, nonzero_identity, persist_state, require_market_unchanged,
    },
    frame::require_distinct,
    records::{authenticate_content_addressed_record, authenticate_finalized_record},
};

/// Exact Resolution role request after the 280-byte Core envelope.
pub const RESOLUTION_ROLE_REQUEST_BYTES_V1: usize =
    dclutch_market_core_codec::CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1
        + RESOLUTION_CORE_ROLE_REQUEST_BYTES;
/// Exact top-level Core data width for a Resolution child effect.
pub const RESOLUTION_CORE_INSTRUCTION_BYTES_V1: usize = dclutch_market_core_codec::REQUEST_BYTES
    + dclutch_market_core_codec::CORE_EFFECT_ENVELOPE_BYTES_V1
    + RESOLUTION_ROLE_REQUEST_BYTES_V1;

/// Exact outer account count for Source/Funding creation.
pub const RESOLUTION_CREATE_OUTER_ACCOUNT_COUNT_V1: usize = 18;
/// Exact outer account count for Source/Funding readiness.
pub const RESOLUTION_VERIFY_OUTER_ACCOUNT_COUNT_V1: usize = 19;
/// Exact child account count for terminal admission.
pub const RESOLUTION_ADMIT_CHILD_ACCOUNT_COUNT_V1: usize = 18;
/// Exact outer account count for terminal admission, including four Core-only Product records.
pub const RESOLUTION_ADMIT_OUTER_ACCOUNT_COUNT_V1: usize = 22;
/// Exact outer and child account count for Source/Funding close.
pub const RESOLUTION_CLOSE_OUTER_ACCOUNT_COUNT_V1: usize = 22;

const SOURCE_MATERIAL: usize = 8;
const SOURCE_MATERIAL_STAGING: usize = 9;
const CAPABILITY_MANIFEST: usize = 10;
const CAPABILITY_MANIFEST_STAGING: usize = 11;
const SOURCE_STATE: usize = 12;
const RECOVERY_FUNDING: usize = 13;
const EXHAUSTION_FUNDING: usize = 14;
const FAILURE_FUNDING: usize = 15;

const CREATE_RENT: usize = 16;
const CREATE_SYSTEM: usize = 17;
const VERIFY_BENEFICIARY: usize = 16;
const VERIFY_CLOCK: usize = 17;
const VERIFY_RENT: usize = 18;
const ADMIT_CERTIFICATE: usize = 16;
const ADMIT_RENT: usize = 17;
const ADMIT_PRODUCT_INSTANCE: usize = 18;
const ADMIT_PRODUCT_INSTANCE_STAGING: usize = 19;
const ADMIT_PRODUCT_TERMS: usize = 20;
const ADMIT_PRODUCT_TERMS_STAGING: usize = 21;
const CLOSE_CERTIFICATE: usize = 16;
const CLOSE_CLOSURE: usize = 17;
const CLOSE_BENEFICIARY: usize = 18;
const CLOSE_CLOCK: usize = 19;
const CLOSE_RENT: usize = 20;
const CLOSE_SYSTEM: usize = 21;

/// Execute one exact Resolution child effect and commit any Core transition last.
#[inline(never)]
pub(crate) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: Request,
    envelope: CoreEffectEnvelopeV1,
    envelope_bytes: &[u8],
    role_request: &[u8],
) -> Result<(), ProgramError> {
    if role_request.len() != RESOLUTION_ROLE_REQUEST_BYTES_V1 {
        return Err(CoreSbfError::Instruction.into());
    }
    let funding_header = CapabilityFundingHeaderV1::decode(
        role_request
            .get(..dclutch_market_core_codec::CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1)
            .ok_or(CoreSbfError::Instruction)?,
    )
    .map_err(|_| CoreSbfError::Instruction)?;
    if funding_header.funding_count() != 3 {
        return Err(CoreSbfError::Instruction.into());
    }
    let resolution_request = ResolutionRoleRequestV1::decode(
        role_request
            .get(dclutch_market_core_codec::CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1..)
            .ok_or(CoreSbfError::Instruction)?,
    )
    .map_err(|_| CoreSbfError::Instruction)?;
    authenticate_action(request, envelope, resolution_request)?;
    validate_outer_frame(program_id, accounts, resolution_request.action)?;
    let frame = FixedRoleAccountsV1::parse(program_id, accounts)?;
    let authenticated = authenticate_fixed_role(
        program_id,
        &frame,
        request,
        envelope,
        role_request,
        Role::Resolution,
    )?;
    authenticate_request_coordinates(&frame, *authenticated.state, envelope, resolution_request)?;
    let admit_projection = if resolution_request.action == ResolutionCoreActionV1::AdmitTerminal {
        Some(authenticate_admit_projection(
            &frame,
            accounts,
            *authenticated.state,
            resolution_request,
        )?)
    } else {
        None
    };
    let close_projection = if resolution_request.action == ResolutionCoreActionV1::CloseFund {
        Some(authenticate_close_prestate(
            &frame,
            accounts,
            *authenticated.state,
            resolution_request,
        )?)
    } else {
        None
    };
    let child_account_count = child_account_count(resolution_request.action);
    invoke_fixed_role(
        program_id,
        &frame,
        envelope,
        envelope_bytes,
        role_request,
        child_account_count,
    )?;
    let acknowledgement =
        authenticate_fixed_role_ack(&frame, envelope, envelope_bytes, role_request)?;
    require_market_unchanged(&frame, authenticated.state_bytes.as_ref())?;
    authenticate_poststate(
        &frame,
        accounts,
        *authenticated.state,
        resolution_request,
        acknowledgement,
        close_projection,
    )?;

    let mut candidate = *authenticated.state;
    match resolution_request.action {
        ResolutionCoreActionV1::CreateFund => {}
        ResolutionCoreActionV1::VerifyFundReady => {
            verify_readiness(
                request,
                &mut candidate,
                *authenticated.core_admission,
                true,
                complete_child_effect(),
            )
            .map_err(|_| CoreSbfError::Transition)?;
            persist_state(frame.market(), candidate)?;
        }
        ResolutionCoreActionV1::AdmitTerminal => {
            let projection = admit_projection.ok_or(CoreSbfError::Transition)?;
            admit_terminal(
                request,
                &mut candidate,
                *authenticated.target_admission,
                projection.product,
                true,
                projection.receipt,
            )
            .map_err(|_| CoreSbfError::Transition)?;
            persist_state(frame.market(), candidate)?;
        }
        ResolutionCoreActionV1::CloseFund => {
            // CloseFund is one authenticated component of eventual Retire.
            // Claims and Custody closure are still required before Core may
            // transition Retiring -> Retired.
        }
    }
    Ok(())
}

#[inline(never)]
fn authenticate_action(
    request: Request,
    envelope: CoreEffectEnvelopeV1,
    resolution: ResolutionRoleRequestV1,
) -> Result<(), CoreSbfError> {
    let (top_level, effect) = match resolution.action {
        ResolutionCoreActionV1::CreateFund => {
            (Action::VerifyReadiness, CoreEffectActionV1::CreateFund)
        }
        ResolutionCoreActionV1::VerifyFundReady => {
            (Action::VerifyReadiness, CoreEffectActionV1::VerifyFundReady)
        }
        ResolutionCoreActionV1::AdmitTerminal => {
            (Action::AdmitTerminal, CoreEffectActionV1::AdmitTerminal)
        }
        ResolutionCoreActionV1::CloseFund => (Action::Retire, CoreEffectActionV1::CloseFund),
    };
    if request.action != top_level
        || envelope.action() != effect
        || envelope.target_role() != Role::Resolution
        || envelope.context().to_bytes() != resolution.source_state
    {
        return Err(CoreSbfError::Instruction);
    }
    let revisions_match = match resolution.action {
        ResolutionCoreActionV1::CreateFund | ResolutionCoreActionV1::VerifyFundReady => {
            envelope.expected_resource_a_revision() == 0
                && envelope.expected_resource_b_revision() == 0
        }
        ResolutionCoreActionV1::AdmitTerminal => {
            envelope.expected_resource_a_revision() == resolution.receipt_sequence
                && envelope.expected_resource_b_revision() == 1
        }
        ResolutionCoreActionV1::CloseFund => {
            envelope.expected_resource_a_revision().checked_add(1)
                == Some(resolution.receipt_sequence)
                && envelope.expected_resource_b_revision() == 1
        }
    };
    if !revisions_match {
        return Err(CoreSbfError::Instruction);
    }
    Ok(())
}

#[inline(never)]
fn authenticate_request_coordinates(
    frame: &FixedRoleAccountsV1<'_, '_>,
    state: CoreState,
    envelope: CoreEffectEnvelopeV1,
    request: ResolutionRoleRequestV1,
) -> Result<(), CoreSbfError> {
    if request.source_state
        != account(frame.child_accounts(16)?, SOURCE_STATE)?
            .key
            .to_bytes()
        || request.recovery_funding
            != account(frame.child_accounts(16)?, RECOVERY_FUNDING)?
                .key
                .to_bytes()
        || request.exhaustion_funding
            != account(frame.child_accounts(16)?, EXHAUSTION_FUNDING)?
                .key
                .to_bytes()
        || request.failure_funding
            != account(frame.child_accounts(16)?, FAILURE_FUNDING)?
                .key
                .to_bytes()
        || request.source_material != state.identity.resolution_policy.to_bytes()
        || request.capability_manifest != state.identity.capability_manifest.to_bytes()
        || envelope.release_set() != state.identity.selected_release_set
    {
        return Err(CoreSbfError::Reference);
    }
    let beneficiary_matches = match request.action {
        ResolutionCoreActionV1::CreateFund => {
            request.beneficiary == state.rent_beneficiary.to_bytes()
        }
        ResolutionCoreActionV1::VerifyFundReady => {
            request.beneficiary == state.rent_beneficiary.to_bytes()
                && request.beneficiary
                    == account(
                        frame.child_accounts(RESOLUTION_VERIFY_OUTER_ACCOUNT_COUNT_V1)?,
                        VERIFY_BENEFICIARY,
                    )?
                    .key
                    .to_bytes()
        }
        ResolutionCoreActionV1::AdmitTerminal => request.beneficiary == [0; 32],
        ResolutionCoreActionV1::CloseFund => {
            request.beneficiary == state.rent_beneficiary.to_bytes()
                && request.beneficiary
                    == account(
                        frame.child_accounts(RESOLUTION_CLOSE_OUTER_ACCOUNT_COUNT_V1)?,
                        CLOSE_BENEFICIARY,
                    )?
                    .key
                    .to_bytes()
        }
    };
    if !beneficiary_matches {
        return Err(CoreSbfError::Reference);
    }
    let valid_phase = match request.action {
        ResolutionCoreActionV1::CreateFund | ResolutionCoreActionV1::VerifyFundReady => {
            state.phase == dclutch_market_core_codec::Phase::Founding
                && state.readiness == Readiness::Prepaid
        }
        ResolutionCoreActionV1::AdmitTerminal => {
            state.phase == dclutch_market_core_codec::Phase::Open
                && state.readiness == Readiness::Consumed
        }
        ResolutionCoreActionV1::CloseFund => {
            state.phase == dclutch_market_core_codec::Phase::Retiring
                && state.readiness == Readiness::Consumed
        }
    };
    if !valid_phase {
        return Err(CoreSbfError::Transition);
    }
    Ok(())
}

#[inline(never)]
fn validate_outer_frame(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    action: ResolutionCoreActionV1,
) -> Result<(), CoreSbfError> {
    let expected = outer_account_count(action);
    if accounts.len() != expected || accounts.iter().any(|value| value.is_signer) {
        return Err(CoreSbfError::AccountFrame);
    }
    require_distinct(accounts)?;
    let common = FixedRoleAccountsV1::parse(program_id, accounts)?;
    if common.market().owner != program_id {
        return Err(CoreSbfError::Market);
    }
    for index in [
        SOURCE_MATERIAL,
        SOURCE_MATERIAL_STAGING,
        CAPABILITY_MANIFEST,
        CAPABILITY_MANIFEST_STAGING,
    ] {
        let value = account(accounts, index)?;
        if value.is_writable || value.executable {
            return Err(CoreSbfError::AccountFrame);
        }
    }
    let expected_writable = match action {
        ResolutionCoreActionV1::CreateFund => [true, true, true, true],
        ResolutionCoreActionV1::VerifyFundReady => [false, true, true, true],
        ResolutionCoreActionV1::AdmitTerminal => [false, false, false, false],
        ResolutionCoreActionV1::CloseFund => [true, true, true, true],
    };
    for (index, writable) in [
        SOURCE_STATE,
        RECOVERY_FUNDING,
        EXHAUSTION_FUNDING,
        FAILURE_FUNDING,
    ]
    .into_iter()
    .zip(expected_writable)
    {
        let value = account(accounts, index)?;
        if value.is_writable != writable || value.executable {
            return Err(CoreSbfError::AccountFrame);
        }
    }
    match action {
        ResolutionCoreActionV1::CreateFund => {
            require_sysvar(account(accounts, CREATE_RENT)?, sysvar::rent::ID)?;
            require_program(account(accounts, CREATE_SYSTEM)?, system_program::ID)?;
        }
        ResolutionCoreActionV1::VerifyFundReady => {
            let beneficiary = account(accounts, VERIFY_BENEFICIARY)?;
            if !beneficiary.is_writable || beneficiary.executable {
                return Err(CoreSbfError::AccountFrame);
            }
            require_sysvar(account(accounts, VERIFY_CLOCK)?, sysvar::clock::ID)?;
            require_sysvar(account(accounts, VERIFY_RENT)?, sysvar::rent::ID)?;
        }
        ResolutionCoreActionV1::AdmitTerminal => {
            let certificate = account(accounts, ADMIT_CERTIFICATE)?;
            if certificate.is_writable || certificate.executable {
                return Err(CoreSbfError::AccountFrame);
            }
            require_sysvar(account(accounts, ADMIT_RENT)?, sysvar::rent::ID)?;
            for index in [
                ADMIT_PRODUCT_INSTANCE,
                ADMIT_PRODUCT_INSTANCE_STAGING,
                ADMIT_PRODUCT_TERMS,
                ADMIT_PRODUCT_TERMS_STAGING,
            ] {
                let value = account(accounts, index)?;
                if value.is_writable || value.executable {
                    return Err(CoreSbfError::AccountFrame);
                }
            }
        }
        ResolutionCoreActionV1::CloseFund => {
            if account(accounts, CLOSE_CERTIFICATE)?.is_writable
                || !account(accounts, CLOSE_CLOSURE)?.is_writable
                || !account(accounts, CLOSE_BENEFICIARY)?.is_writable
            {
                return Err(CoreSbfError::AccountFrame);
            }
            require_sysvar(account(accounts, CLOSE_CLOCK)?, sysvar::clock::ID)?;
            require_sysvar(account(accounts, CLOSE_RENT)?, sysvar::rent::ID)?;
            require_program(account(accounts, CLOSE_SYSTEM)?, system_program::ID)?;
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct AdmitProjection {
    product: Product,
    receipt: TerminalReceipt,
}

#[derive(Clone, Copy)]
struct CloseProjection {
    source_state_digest: [u8; 32],
    terminal_certificate_digest: [u8; 32],
    funding_set_digest: [u8; 32],
}

#[inline(never)]
fn authenticate_close_prestate(
    frame: &FixedRoleAccountsV1<'_, '_>,
    accounts: &[AccountInfo<'_>],
    state: CoreState,
    request: ResolutionRoleRequestV1,
) -> Result<CloseProjection, CoreSbfError> {
    let rent = read_rent(account(accounts, CLOSE_RENT)?)?;
    authenticate_live_poststate(
        frame,
        accounts,
        state,
        request,
        ResolutionCoreActionV1::AdmitTerminal,
        &rent,
    )?;
    let source = account(accounts, SOURCE_STATE)?
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Reference)?;
    let certificate_account = account(accounts, CLOSE_CERTIFICATE)?;
    let certificate = certificate_account
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Reference)?;
    if certificate_account.owner != frame.target_program().key
        || certificate_account.data_len() != RESOLUTION_CERTIFICATE_BYTES
        || certificate_account.key.to_bytes()
            != state
                .terminal_receipt
                .ok_or(CoreSbfError::Transition)?
                .to_bytes()
        || !rent.is_exempt(certificate_account.lamports(), RESOLUTION_CERTIFICATE_BYTES)
    {
        return Err(CoreSbfError::Reference);
    }
    let decoded =
        ResolutionCertificateV1::decode(&certificate).map_err(|_| CoreSbfError::Reference)?;
    if decoded.market != frame.market().key.to_bytes()
        || decoded.source_material != state.identity.resolution_policy.to_bytes()
        || decoded.product != state.identity.product_id.to_bytes()
        || decoded.receipt_account != certificate_account.key.to_bytes()
        || decoded.generation != state.identity.generation
        || decoded.selector != state.terminal_winner
    {
        return Err(CoreSbfError::Reference);
    }
    let recovery = account(accounts, RECOVERY_FUNDING)?
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Reference)?;
    let exhaustion = account(accounts, EXHAUSTION_FUNDING)?
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Reference)?;
    let failure = account(accounts, FAILURE_FUNDING)?
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Reference)?;
    Ok(CloseProjection {
        source_state_digest: solana_program::hash::hash(&source).to_bytes(),
        terminal_certificate_digest: solana_program::hash::hash(&certificate).to_bytes(),
        funding_set_digest: hashv(&[
            SOURCE_FUNDING_SET_DIGEST_DOMAIN_V1,
            &recovery,
            &exhaustion,
            &failure,
        ])
        .to_bytes(),
    })
}

#[inline(never)]
fn authenticate_admit_projection(
    frame: &FixedRoleAccountsV1<'_, '_>,
    accounts: &[AccountInfo<'_>],
    state: CoreState,
    request: ResolutionRoleRequestV1,
) -> Result<AdmitProjection, CoreSbfError> {
    let rent = read_rent(account(accounts, ADMIT_RENT)?)?;
    let registry = frame.registry().key;
    let material_account = account(accounts, SOURCE_MATERIAL)?;
    let material_data = material_account
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    if material_data.len() != SOURCE_MATERIAL_BYTES {
        return Err(CoreSbfError::Reference);
    }
    authenticate_finalized_record(
        registry,
        material_account,
        account(accounts, SOURCE_MATERIAL_STAGING)?,
        &rent,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1,
        request.source_material,
        &material_data,
    )?;
    let material =
        SourceMaterialViewV1::decode(&material_data).map_err(|_| CoreSbfError::Reference)?;
    let domain = material
        .result_domain()
        .map_err(|_| CoreSbfError::Reference)?;
    let domain_bytes = domain.to_bytes();
    let result_domain_id =
        hashv(&[FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1, &[0], &domain_bytes]).to_bytes();

    let instance_account = account(accounts, ADMIT_PRODUCT_INSTANCE)?;
    let instance_data = instance_account
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    if instance_data.len() != INSTANCE_BYTES {
        return Err(CoreSbfError::Reference);
    }
    let (product_id, instance_bytes) = authenticate_content_addressed_record(
        registry,
        instance_account,
        account(accounts, ADMIT_PRODUCT_INSTANCE_STAGING)?,
        &rent,
        PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1,
        &instance_data,
    )?;
    let instance = InstanceV1::decode(instance_bytes).map_err(|_| CoreSbfError::Reference)?;
    let terms_account = account(accounts, ADMIT_PRODUCT_TERMS)?;
    let terms_data = terms_account
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    if terms_data.len() != TERMS_BYTES {
        return Err(CoreSbfError::Reference);
    }
    authenticate_finalized_record(
        registry,
        terms_account,
        account(accounts, ADMIT_PRODUCT_TERMS_STAGING)?,
        &rent,
        PRODUCT_TERMS_SCHEMA_RELEASE_ID_V1,
        instance.terms_id().to_bytes(),
        &terms_data,
    )?;
    let terms = TermsV1::decode(&terms_data).map_err(|_| CoreSbfError::Reference)?;
    if product_id != state.identity.product_id.to_bytes()
        || material
            .product_instance_id()
            .map_err(|_| CoreSbfError::Reference)?
            .to_bytes()
            != product_id
        || instance.result_domain_id().to_bytes() != result_domain_id
        || state.identity.result_domain.to_bytes() != result_domain_id
        || instance.capacity_profile_id() != terms.capacity_profile_id()
        || instance.partition_cell_count() != u32::from(domain.outcome_count())
        || instance.partition_cell_count() != terms.partition_cell_count()
    {
        return Err(CoreSbfError::Reference);
    }
    let product = Product {
        product_id: nonzero_identity(product_id)?,
        result_domain: nonzero_identity(result_domain_id)?,
        claim_basis: nonzero_identity(instance.claim_basis_id().to_bytes())?,
        capacity_profile: nonzero_identity(instance.capacity_profile_id().content_id().to_bytes())?,
        compiler_release: nonzero_identity(terms.semantic_release_id().to_bytes())?,
        outcome_count: u32::from(domain.outcome_count()),
    };

    let source_account = account(accounts, SOURCE_STATE)?;
    let source_data = source_account
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Reference)?;
    let source =
        SourceResolutionStateV1::decode(&source_data).map_err(|_| CoreSbfError::Reference)?;
    authenticate_source_state(frame.target_program().key, source_account, source)?;
    if !matches!(
        source.phase(),
        SourceResolutionPhaseV1::Resolved | SourceResolutionPhaseV1::FailureCommitted
    ) || source.market() != frame.market().key.to_bytes()
        || source.generation() != state.identity.generation
        || source.material_id().to_bytes() != request.source_material
    {
        return Err(CoreSbfError::Reference);
    }
    let decision = source
        .decision(domain.outcome_count())
        .map_err(|_| CoreSbfError::Transition)?;
    if decision.terminal_sequence() != request.receipt_sequence {
        return Err(CoreSbfError::Transition);
    }
    let certificate_account = account(accounts, ADMIT_CERTIFICATE)?;
    let certificate_data = certificate_account
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Reference)?;
    let certificate = authenticate_terminal_certificate(
        frame.target_program().key,
        source_account,
        certificate_account,
        request,
        state,
        decision.selector(),
        &certificate_data,
        &rent,
    )?;
    Ok(AdmitProjection {
        product,
        receipt: TerminalReceipt {
            receipt_id: nonzero_identity(certificate_account.key.to_bytes())?,
            market_id: state.identity.market_id,
            resolution_policy: state.identity.resolution_policy,
            product_id: state.identity.product_id,
            generation: state.identity.generation,
            selector: certificate.selector,
            authenticated: true,
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn authenticate_terminal_certificate(
    resolution_program: &Pubkey,
    source_account: &AccountInfo<'_>,
    certificate_account: &AccountInfo<'_>,
    request: ResolutionRoleRequestV1,
    state: CoreState,
    selector: u8,
    bytes: &[u8],
    rent: &Rent,
) -> Result<ResolutionCertificateV1, CoreSbfError> {
    let (expected_kind, kind_tag) = match request.receipt_kind {
        ResolutionCoreReceiptKindV1::TerminalSuccess => {
            (ResolutionCertificateKindV1::ResolutionSuccess, 1_u8)
        }
        ResolutionCoreReceiptKindV1::TerminalFailure => {
            (ResolutionCertificateKindV1::ResolutionFailure, 4_u8)
        }
        ResolutionCoreReceiptKindV1::None | ResolutionCoreReceiptKindV1::Closure => {
            return Err(CoreSbfError::Reference);
        }
    };
    if certificate_account.key.to_bytes() != request.receipt
        || certificate_account.owner != resolution_program
        || certificate_account.data_len() != RESOLUTION_CERTIFICATE_BYTES
        || !rent.is_exempt(certificate_account.lamports(), RESOLUTION_CERTIFICATE_BYTES)
    {
        return Err(CoreSbfError::Reference);
    }
    let certificate =
        ResolutionCertificateV1::decode(bytes).map_err(|_| CoreSbfError::Reference)?;
    if certificate.kind != expected_kind
        || certificate.market != state.identity.market_id.to_bytes()
        || certificate.source_material != request.source_material
        || certificate.product != state.identity.product_id.to_bytes()
        || certificate.receipt_account != certificate_account.key.to_bytes()
        || certificate.generation != state.identity.generation
        || certificate.selector != u32::from(selector)
    {
        return Err(CoreSbfError::Reference);
    }
    let kind = [kind_tag];
    let sequence = request.receipt_sequence.to_le_bytes();
    let expected = Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            source_account.key.as_ref(),
            &kind,
            &sequence,
        ],
        resolution_program,
    )
    .0;
    if certificate_account.key != &expected {
        return Err(CoreSbfError::Reference);
    }
    Ok(certificate)
}

#[inline(never)]
fn authenticate_poststate(
    frame: &FixedRoleAccountsV1<'_, '_>,
    accounts: &[AccountInfo<'_>],
    state: CoreState,
    request: ResolutionRoleRequestV1,
    acknowledgement: CoreEffectAckV1,
    close_projection: Option<CloseProjection>,
) -> Result<(), CoreSbfError> {
    let rent = read_rent(account(accounts, rent_index(request.action))?)?;
    let observed_digest = match request.action {
        ResolutionCoreActionV1::CloseFund => {
            authenticate_close_poststate(
                frame,
                accounts,
                state,
                request,
                close_projection.ok_or(CoreSbfError::ChildAck)?,
                &rent,
            )?;
            let closure = account(accounts, CLOSE_CLOSURE)?
                .try_borrow_data()
                .map_err(|_| CoreSbfError::ChildAck)?;
            resolution_poststate_digest(request.action, &closure, &[], &[], &[], None)?
        }
        action => {
            authenticate_live_poststate(frame, accounts, state, request, action, &rent)?;
            let source = account(accounts, SOURCE_STATE)?
                .try_borrow_data()
                .map_err(|_| CoreSbfError::ChildAck)?;
            let recovery = account(accounts, RECOVERY_FUNDING)?
                .try_borrow_data()
                .map_err(|_| CoreSbfError::ChildAck)?;
            let exhaustion = account(accounts, EXHAUSTION_FUNDING)?
                .try_borrow_data()
                .map_err(|_| CoreSbfError::ChildAck)?;
            let failure = account(accounts, FAILURE_FUNDING)?
                .try_borrow_data()
                .map_err(|_| CoreSbfError::ChildAck)?;
            let certificate = if action == ResolutionCoreActionV1::AdmitTerminal {
                Some(
                    account(accounts, ADMIT_CERTIFICATE)?
                        .try_borrow_data()
                        .map_err(|_| CoreSbfError::ChildAck)?,
                )
            } else {
                None
            };
            resolution_poststate_digest(
                action,
                &source,
                &recovery,
                &exhaustion,
                &failure,
                certificate.as_ref().map(|bytes| bytes.as_ref()),
            )?
        }
    };
    if acknowledgement.post_resource_digest() != observed_digest
        || !ack_revisions_match(acknowledgement, request)
    {
        return Err(CoreSbfError::ChildAck);
    }
    Ok(())
}

fn authenticate_live_poststate(
    frame: &FixedRoleAccountsV1<'_, '_>,
    accounts: &[AccountInfo<'_>],
    state: CoreState,
    request: ResolutionRoleRequestV1,
    action: ResolutionCoreActionV1,
    rent: &Rent,
) -> Result<(), CoreSbfError> {
    let source_account = account(accounts, SOURCE_STATE)?;
    if source_account.owner != frame.target_program().key
        || source_account.data_len() != SOURCE_RESOLUTION_STATE_BYTES
    {
        return Err(CoreSbfError::ChildAck);
    }
    let source_data = source_account
        .try_borrow_data()
        .map_err(|_| CoreSbfError::ChildAck)?;
    let source =
        SourceResolutionStateV1::decode(&source_data).map_err(|_| CoreSbfError::ChildAck)?;
    authenticate_source_state(frame.target_program().key, source_account, source)?;
    if source.market() != frame.market().key.to_bytes()
        || source.generation() != state.identity.generation
        || source.material_id().to_bytes() != request.source_material
        || !matches!(
            (action, source.phase()),
            (
                ResolutionCoreActionV1::CreateFund | ResolutionCoreActionV1::VerifyFundReady,
                SourceResolutionPhaseV1::Primary
            ) | (
                ResolutionCoreActionV1::AdmitTerminal,
                SourceResolutionPhaseV1::Resolved | SourceResolutionPhaseV1::FailureCommitted
            )
        )
    {
        return Err(CoreSbfError::ChildAck);
    }
    let manifest_account = account(accounts, CAPABILITY_MANIFEST)?;
    let manifest_data = manifest_account
        .try_borrow_data()
        .map_err(|_| CoreSbfError::ChildAck)?;
    authenticate_finalized_record(
        frame.registry().key,
        manifest_account,
        account(accounts, CAPABILITY_MANIFEST_STAGING)?,
        rent,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        request.capability_manifest,
        &manifest_data,
    )?;
    let manifest =
        CapabilityManifestV1::decode(&manifest_data).map_err(|_| CoreSbfError::Funding)?;
    let manifest_id =
        CapabilityContentId::new(request.capability_manifest).map_err(|_| CoreSbfError::Funding)?;
    let expected_status = if action == ResolutionCoreActionV1::CreateFund {
        FundingStatus::Pending
    } else {
        FundingStatus::Active
    };
    for (index, entry_index) in [
        (RECOVERY_FUNDING, request.recovery_entry_index),
        (EXHAUSTION_FUNDING, request.exhaustion_entry_index),
        (FAILURE_FUNDING, request.failure_entry_index),
    ] {
        let funding_account = account(accounts, index)?;
        if funding_account.owner != frame.target_program().key
            || funding_account.data_len() != FUNDING_STATE_BYTES
        {
            return Err(CoreSbfError::ChildAck);
        }
        let funding_data = funding_account
            .try_borrow_data()
            .map_err(|_| CoreSbfError::ChildAck)?;
        let funding = FundingStateV1::decode(&funding_data).map_err(|_| CoreSbfError::Funding)?;
        if funding.entry_index() != entry_index || funding.status() != expected_status {
            return Err(CoreSbfError::ChildAck);
        }
        let custody = FundingCustodyObservationV1::native_only(
            funding_account.lamports(),
            rent.minimum_balance(FUNDING_STATE_BYTES),
        )
        .map_err(|_| CoreSbfError::Funding)?;
        funding
            .validate_against(manifest_id, manifest, custody)
            .map_err(|_| CoreSbfError::Funding)?;
        let derivation = CapabilityFundingDerivationV1::new(
            frame.market().key.to_bytes(),
            state.identity.generation,
            manifest_id,
            manifest,
            funding,
        )
        .map_err(|_| CoreSbfError::Funding)?;
        if Pubkey::find_program_address(&derivation.seed_components(), frame.target_program().key).0
            != *funding_account.key
        {
            return Err(CoreSbfError::Funding);
        }
    }
    Ok(())
}

fn authenticate_close_poststate(
    frame: &FixedRoleAccountsV1<'_, '_>,
    accounts: &[AccountInfo<'_>],
    state: CoreState,
    request: ResolutionRoleRequestV1,
    prestate: CloseProjection,
    rent: &Rent,
) -> Result<(), CoreSbfError> {
    for index in [
        SOURCE_STATE,
        RECOVERY_FUNDING,
        EXHAUSTION_FUNDING,
        FAILURE_FUNDING,
    ] {
        let value = account(accounts, index)?;
        let data = value
            .try_borrow_data()
            .map_err(|_| CoreSbfError::ChildAck)?;
        if value.owner != frame.target_program().key
            || value.lamports() != 0
            || data.iter().any(|byte| *byte != 0)
        {
            return Err(CoreSbfError::ChildAck);
        }
    }
    let closure_account = account(accounts, CLOSE_CLOSURE)?;
    let closure_data = closure_account
        .try_borrow_data()
        .map_err(|_| CoreSbfError::ChildAck)?;
    if closure_account.owner != frame.target_program().key
        || closure_account.key.to_bytes() != request.receipt
        || closure_account.data_len() != SOURCE_CLOSURE_RECEIPT_BYTES
        || !rent.is_exempt(closure_account.lamports(), SOURCE_CLOSURE_RECEIPT_BYTES)
    {
        return Err(CoreSbfError::ChildAck);
    }
    let closure =
        SourceClosureReceiptV1::decode(&closure_data).map_err(|_| CoreSbfError::ChildAck)?;
    let terminal_receipt = state
        .terminal_receipt
        .ok_or(CoreSbfError::Transition)?
        .to_bytes();
    if closure.market != frame.market().key.to_bytes()
        || closure.source_state != account(accounts, SOURCE_STATE)?.key.to_bytes()
        || closure.source_material != request.source_material
        || closure.capability_manifest != request.capability_manifest
        || closure.terminal_certificate != terminal_receipt
        || closure.receipt_account != closure_account.key.to_bytes()
        || closure.beneficiary != request.beneficiary
        || closure.generation != state.identity.generation
        || closure
            .terminal_sequence
            .checked_add(1)
            .ok_or(CoreSbfError::Arithmetic)?
            != request.receipt_sequence
        || closure.selector != state.terminal_winner
        || closure.source_state_digest != prestate.source_state_digest
        || closure.terminal_certificate_digest != prestate.terminal_certificate_digest
        || closure.funding_set_digest != prestate.funding_set_digest
    {
        return Err(CoreSbfError::ChildAck);
    }
    let sequence = request.receipt_sequence.to_le_bytes();
    if Pubkey::find_program_address(
        &[
            SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V1,
            account(accounts, SOURCE_STATE)?.key.as_ref(),
            &sequence,
        ],
        frame.target_program().key,
    )
    .0 != *closure_account.key
    {
        return Err(CoreSbfError::ChildAck);
    }
    Ok(())
}

fn authenticate_source_state(
    resolution_program: &Pubkey,
    account: &AccountInfo<'_>,
    state: SourceResolutionStateV1,
) -> Result<(), CoreSbfError> {
    let seeds = state.pda_seeds();
    let bump = [seeds.bump()];
    let expected = Pubkey::create_program_address(
        &[
            seeds.domain(),
            &seeds.market(),
            &seeds.generation_le(),
            &bump,
        ],
        resolution_program,
    )
    .map_err(|_| CoreSbfError::Reference)?;
    if account.key != &expected {
        return Err(CoreSbfError::Reference);
    }
    Ok(())
}

fn resolution_poststate_digest(
    action: ResolutionCoreActionV1,
    source_or_closure: &[u8],
    recovery: &[u8],
    exhaustion: &[u8],
    failure: &[u8],
    certificate: Option<&[u8]>,
) -> Result<dclutch_market_core_codec::Identity, CoreSbfError> {
    let action_tag = [match action {
        ResolutionCoreActionV1::CreateFund => CoreEffectActionV1::CreateFund as u8,
        ResolutionCoreActionV1::VerifyFundReady => CoreEffectActionV1::VerifyFundReady as u8,
        ResolutionCoreActionV1::AdmitTerminal => CoreEffectActionV1::AdmitTerminal as u8,
        ResolutionCoreActionV1::CloseFund => CoreEffectActionV1::CloseFund as u8,
    }];
    nonzero_identity(
        hashv(&[
            RESOLUTION_POSTSTATE_DIGEST_DOMAIN_V1,
            &action_tag,
            source_or_closure,
            recovery,
            exhaustion,
            failure,
            certificate.unwrap_or(&[]),
        ])
        .to_bytes(),
    )
}

fn ack_revisions_match(acknowledgement: CoreEffectAckV1, request: ResolutionRoleRequestV1) -> bool {
    match request.action {
        ResolutionCoreActionV1::CreateFund => {
            acknowledgement.pre_resource_a_revision() == 0
                && acknowledgement.post_resource_a_revision() == 0
                && acknowledgement.pre_resource_b_revision() == 0
                && acknowledgement.post_resource_b_revision() == 0
        }
        ResolutionCoreActionV1::VerifyFundReady => {
            acknowledgement.pre_resource_a_revision() == 0
                && acknowledgement.post_resource_a_revision() == 0
                && acknowledgement.pre_resource_b_revision() == 0
                && acknowledgement.post_resource_b_revision() == 1
        }
        ResolutionCoreActionV1::AdmitTerminal => {
            acknowledgement.pre_resource_a_revision() == request.receipt_sequence
                && acknowledgement.post_resource_a_revision() == request.receipt_sequence
                && acknowledgement.pre_resource_b_revision() == 1
                && acknowledgement.post_resource_b_revision() == 1
        }
        ResolutionCoreActionV1::CloseFund => {
            acknowledgement.pre_resource_a_revision().checked_add(1)
                == Some(request.receipt_sequence)
                && acknowledgement.post_resource_a_revision() == request.receipt_sequence
                && acknowledgement.pre_resource_b_revision() == 1
                && acknowledgement.post_resource_b_revision() == 2
        }
    }
}

const fn complete_child_effect() -> ChildEffectObservation {
    ChildEffectObservation {
        exact_request_authenticated: true,
        exact_receipt_authenticated: true,
        post_resource_authenticated: true,
    }
}

const fn child_account_count(action: ResolutionCoreActionV1) -> usize {
    match action {
        ResolutionCoreActionV1::CreateFund => RESOLUTION_CREATE_OUTER_ACCOUNT_COUNT_V1,
        ResolutionCoreActionV1::VerifyFundReady => RESOLUTION_VERIFY_OUTER_ACCOUNT_COUNT_V1,
        ResolutionCoreActionV1::AdmitTerminal => RESOLUTION_ADMIT_CHILD_ACCOUNT_COUNT_V1,
        ResolutionCoreActionV1::CloseFund => RESOLUTION_CLOSE_OUTER_ACCOUNT_COUNT_V1,
    }
}

const fn outer_account_count(action: ResolutionCoreActionV1) -> usize {
    match action {
        ResolutionCoreActionV1::CreateFund => RESOLUTION_CREATE_OUTER_ACCOUNT_COUNT_V1,
        ResolutionCoreActionV1::VerifyFundReady => RESOLUTION_VERIFY_OUTER_ACCOUNT_COUNT_V1,
        ResolutionCoreActionV1::AdmitTerminal => RESOLUTION_ADMIT_OUTER_ACCOUNT_COUNT_V1,
        ResolutionCoreActionV1::CloseFund => RESOLUTION_CLOSE_OUTER_ACCOUNT_COUNT_V1,
    }
}

const fn rent_index(action: ResolutionCoreActionV1) -> usize {
    match action {
        ResolutionCoreActionV1::CreateFund => CREATE_RENT,
        ResolutionCoreActionV1::VerifyFundReady => VERIFY_RENT,
        ResolutionCoreActionV1::AdmitTerminal => ADMIT_RENT,
        ResolutionCoreActionV1::CloseFund => CLOSE_RENT,
    }
}

fn read_rent(account: &AccountInfo<'_>) -> Result<Rent, CoreSbfError> {
    if account.key != &sysvar::rent::ID || account.owner != &sysvar::ID {
        return Err(CoreSbfError::AccountFrame);
    }
    Rent::from_account_info(account).map_err(|_| CoreSbfError::AccountFrame)
}

fn require_sysvar(account: &AccountInfo<'_>, key: Pubkey) -> Result<(), CoreSbfError> {
    if account.key != &key || account.is_writable || account.executable {
        return Err(CoreSbfError::AccountFrame);
    }
    Ok(())
}

fn require_program(account: &AccountInfo<'_>, key: Pubkey) -> Result<(), CoreSbfError> {
    if account.key != &key || account.is_writable || !account.executable {
        return Err(CoreSbfError::AccountFrame);
    }
    Ok(())
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, CoreSbfError> {
    accounts.get(index).ok_or(CoreSbfError::AccountFrame)
}

const _: usize = SOURCE_MATERIAL_BYTES;
