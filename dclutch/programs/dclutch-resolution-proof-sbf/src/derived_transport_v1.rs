//! Physical route settling a child market from its parents' certificates.
//!
//! The frame is `ParentReferenceV1Abi.settleFrame`: thirty accounts, or
//! thirty-three with parent B's suffix. Each parent is presented as its Core
//! Market, its Resolution-owned Source state and its terminal certificate,
//! and the certificate is authenticated exactly as Core's `AdmitTerminal`
//! authenticates one (`resolution.rs`, `authenticate_terminal_certificate`):
//! owner, width, rent, decode, kind, market, generation, receipt account,
//! and the PDA under this program from the parent's Source state, the kind
//! seed and the parent's own terminal sequence. Nothing of a parent is
//! written (`no_parent_account_is_writable`).

use alloc::boxed::Box;

use dclutch_market::CoreState;
use dclutch_market::MarketCoreStateSeedsV2;
use dclutch_market::capability_manifest::funding::funded_rent_persists_v1;
use dclutch_product::ContentId as ProductContentId;
use dclutch_product::ResultDomainV2;
use dclutch_product::svm_reader::{FinalizedRecordFrameV2, ProductRuntimeFrameV2};
use dclutch_source::parent_reference_v1::{
    DERIVED_SETTLE_ACCOUNT_COUNT_V1, DERIVED_SETTLE_ACCOUNT_ROLES_V1,
    DERIVED_SETTLE_CERTIFICATE_INDEX_V1, DERIVED_SETTLE_CLOCK_INDEX_V1,
    DERIVED_SETTLE_MARKET_INDEX_V1, DERIVED_SETTLE_MATERIAL_RAW_INDEX_V1,
    DERIVED_SETTLE_OFF_CONDITION_ACCOUNT_COUNT_V1, DERIVED_SETTLE_PARENT_A_MARKET_INDEX_V1,
    DERIVED_SETTLE_PARENT_B_MARKET_INDEX_V1, DERIVED_SETTLE_PARENT_REFERENCE_RAW_INDEX_V1,
    DERIVED_SETTLE_PRODUCT_RAW_INDEX_V1, DERIVED_SETTLE_RENT_INDEX_V1,
    DERIVED_SETTLE_SOURCE_STATE_INDEX_V1, DERIVED_SETTLE_SYSTEM_INDEX_V1, DerivedSettleRequestV1,
    PARENT_REFERENCE_BYTES_V1, PARENT_REFERENCE_SCHEMA_ID_V1, ParentCertificateV1,
    ParentReferenceV1, is_derived_settle_v1,
};
use dclutch_source::resolution::{
    RESOLUTION_CERTIFICATE_BYTES_V2, RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
    ResolutionCertificateKindV2, ResolutionCertificateV2,
};
use dclutch_source::{
    ContentId as SourceContentId, PROVIDER_RELEASE_BYTES, PROVIDER_RELEASE_SCHEMA_ID_V1,
    ProviderReleaseV1, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3, SOURCE_MATERIAL_V3_BYTES,
    SOURCE_RESOLUTION_STATE_BYTES_V2, SOURCE_SPEC_BYTES, SOURCE_SPEC_SCHEMA_ID_V1,
    STATISTIC_SPEC_BYTES, STATISTIC_SPEC_SCHEMA_ID_V1, SourceMaterialV3, SourceResolutionPhaseV1,
    SourceResolutionStateV2, SourceSpecV1, StatisticSpecV1, WINDOW_SPEC_BYTES,
    WINDOW_SPEC_SCHEMA_ID_V1, WindowSpecV1,
};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey, rent::Rent,
};

use crate::derived_v1::{
    AuthenticatedDerivedSourceRecordsV1, AuthenticatedParentCertificateV1, DerivedJoinErrorV1,
    DerivedResolutionRequestV1, plan_derived_resolution_v1,
};
use crate::provider_instruction_v3::authenticate_record;
use crate::relay_transport_v1::{
    MarketFacts, account, authenticate_market, authenticate_source_state_account,
    boxed_product_runtime, initialize_certificate_at_kind, require_system,
};
use crate::{ResolutionError, authenticate_clock, authenticate_rent};

/// Return whether bytes select the derived settle instruction.
pub(crate) fn is_derived_settle(bytes: &[u8]) -> bool {
    is_derived_settle_v1(bytes)
}

const fn map_join_error(error: DerivedJoinErrorV1) -> ResolutionError {
    match error {
        DerivedJoinErrorV1::Request => ResolutionError::Instruction,
        DerivedJoinErrorV1::Source => ResolutionError::SourceMaterial,
        DerivedJoinErrorV1::Product => ResolutionError::ProductDomain,
        DerivedJoinErrorV1::Reference => ResolutionError::DerivedReference,
        DerivedJoinErrorV1::ParentNotTerminal => ResolutionError::DerivedParentNotTerminal,
        DerivedJoinErrorV1::WrongParent => ResolutionError::DerivedWrongParent,
        DerivedJoinErrorV1::ParentGeneration => ResolutionError::DerivedParentGeneration,
        DerivedJoinErrorV1::ParentRecord => ResolutionError::DerivedParentRecord,
        DerivedJoinErrorV1::ParentWidth => ResolutionError::DerivedParentWidth,
        DerivedJoinErrorV1::SelectorOutOfRange => ResolutionError::DerivedSelectorOutOfRange,
        DerivedJoinErrorV1::ParentFailed => ResolutionError::DerivedParentFailed,
        DerivedJoinErrorV1::WindowClosed => ResolutionError::DerivedWindowClosed,
        DerivedJoinErrorV1::Transition => ResolutionError::Transition,
        DerivedJoinErrorV1::Arithmetic => ResolutionError::Arithmetic,
    }
}

/// Settle one child market from its parents' terminal certificates.
#[inline(never)]
pub(crate) fn process_derived_settle_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let request = DerivedSettleRequestV1::decode(instruction_data)
        .map_err(|_| ResolutionError::Instruction)?;
    let with_b = validate_frame(accounts)?;
    let worker = account(accounts, 0)?;
    let market_account = account(accounts, DERIVED_SETTLE_MARKET_INDEX_V1)?;
    let core = account(accounts, 2)?;
    let activation = account(accounts, 3)?;
    let source_state_account = account(accounts, DERIVED_SETTLE_SOURCE_STATE_INDEX_V1)?;
    let certificate_account = account(accounts, DERIVED_SETTLE_CERTIFICATE_INDEX_V1)?;
    let clock_account = account(accounts, DERIVED_SETTLE_CLOCK_INDEX_V1)?;
    let rent_sysvar = account(accounts, DERIVED_SETTLE_RENT_INDEX_V1)?;
    let system = account(accounts, DERIVED_SETTLE_SYSTEM_INDEX_V1)?;
    require_system(system)?;
    if !worker.is_signer {
        return Err(ResolutionError::AccountFrame.into());
    }
    let rent = authenticate_rent(rent_sysvar)?;
    let clock = authenticate_clock(clock_account)?;

    let market = authenticate_market(
        program_id,
        market_account,
        core,
        activation,
        request.generation,
        request.source_material,
    )?;
    let records = boxed_source_records(&market, accounts, &request)?;
    let product_runtime = boxed_product_runtime(
        &market.registry_program,
        ProductContentId::new(market.product_record).map_err(|_| ResolutionError::ProductDomain)?,
        ProductRuntimeFrameV2 {
            product: FinalizedRecordFrameV2 {
                raw: account(accounts, DERIVED_SETTLE_PRODUCT_RAW_INDEX_V1)?,
                staging: account(accounts, DERIVED_SETTLE_PRODUCT_RAW_INDEX_V1 + 1)?,
            },
            result_domain: FinalizedRecordFrameV2 {
                raw: account(accounts, DERIVED_SETTLE_PRODUCT_RAW_INDEX_V1 + 2)?,
                staging: account(accounts, DERIVED_SETTLE_PRODUCT_RAW_INDEX_V1 + 3)?,
            },
            portfolio: FinalizedRecordFrameV2 {
                raw: account(accounts, DERIVED_SETTLE_PRODUCT_RAW_INDEX_V1 + 4)?,
                staging: account(accounts, DERIVED_SETTLE_PRODUCT_RAW_INDEX_V1 + 5)?,
            },
        },
    )?;
    authenticate_source_state_account(program_id, source_state_account, market_account)?;

    let parent_a = boxed_parent(
        program_id,
        core.key,
        accounts,
        DERIVED_SETTLE_PARENT_A_MARKET_INDEX_V1,
        records.reference.parent_a.ordinary_count,
    )?;
    let parent_b = if with_b {
        Some(boxed_parent(
            program_id,
            core.key,
            accounts,
            DERIVED_SETTLE_PARENT_B_MARKET_INDEX_V1,
            records.reference.parent_b.ordinary_count,
        )?)
    } else {
        None
    };

    let domain_data = account(accounts, DERIVED_SETTLE_PRODUCT_RAW_INDEX_V1 + 2)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::ProductDomain)?;
    let result_domain =
        ResultDomainV2::decode(&domain_data).map_err(|_| ResolutionError::ProductDomain)?;
    let source_data = source_state_account
        .try_borrow_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let source_state = Box::new(
        SourceResolutionStateV2::decode(&source_data).map_err(|_| ResolutionError::OutputState)?,
    );

    let plan = plan_derived_resolution_v1(
        request,
        &DerivedResolutionRequestV1 {
            market: market_account.key.to_bytes(),
            certificate_account: certificate_account.key.to_bytes(),
            current_unix_seconds: clock.unix_timestamp,
        },
        &source_state,
        &records,
        &product_runtime,
        result_domain,
        &parent_a,
        parent_b.as_deref(),
    )
    .map_err(map_join_error)?;

    let next_source = Box::new(plan.next_source.to_bytes());
    let certificate = Box::new(
        plan.certificate
            .to_bytes()
            .map_err(|_| ResolutionError::Transition)?,
    );
    drop(source_data);
    drop(domain_data);
    commit_settlement(
        program_id,
        request.terminal_sequence,
        source_state_account,
        certificate_account,
        system,
        &rent,
        &next_source,
        &certificate,
    )
}

/// The frame: one of two widths, every slot's privileges the emitted table's,
/// no aliasing among the writable outputs.
fn validate_frame(accounts: &[AccountInfo<'_>]) -> Result<bool, ProgramError> {
    let with_b = match accounts.len() {
        DERIVED_SETTLE_ACCOUNT_COUNT_V1 => true,
        DERIVED_SETTLE_OFF_CONDITION_ACCOUNT_COUNT_V1 => false,
        _ => return Err(ResolutionError::AccountFrame.into()),
    };
    for (index, account) in accounts.iter().enumerate() {
        let (writable, signer) = DERIVED_SETTLE_ACCOUNT_ROLES_V1
            .get(index)
            .copied()
            .ok_or(ResolutionError::AccountFrame)?;
        if account.is_writable != writable || account.is_signer != signer {
            return Err(ResolutionError::AccountFrame.into());
        }
    }
    let source_state = account(accounts, DERIVED_SETTLE_SOURCE_STATE_INDEX_V1)?;
    let certificate = account(accounts, DERIVED_SETTLE_CERTIFICATE_INDEX_V1)?;
    if source_state.key == certificate.key {
        return Err(ResolutionError::AccountFrame.into());
    }
    Ok(with_b)
}

fn boxed_source_records(
    market: &MarketFacts,
    accounts: &[AccountInfo<'_>],
    request: &DerivedSettleRequestV1,
) -> Result<Box<AuthenticatedDerivedSourceRecordsV1>, ProgramError> {
    Ok(Box::new(source_records(market, accounts, request)?))
}

/// The child's Source graph: material, spec, provider release, window,
/// statistic, and the parent reference the spec names, each Registry-finalized
/// under its schema at the digest the previous link committed to.
#[inline(never)]
fn source_records(
    market: &MarketFacts,
    accounts: &[AccountInfo<'_>],
    request: &DerivedSettleRequestV1,
) -> Result<AuthenticatedDerivedSourceRecordsV1, ProgramError> {
    let registry = &market.registry_program;
    let base = DERIVED_SETTLE_MATERIAL_RAW_INDEX_V1;
    let material_data = account(accounts, base)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        account(accounts, base)?,
        account(accounts, base + 1)?,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        request.source_material,
        &material_data,
        SOURCE_MATERIAL_V3_BYTES,
    )?;
    let material =
        SourceMaterialV3::decode(&material_data).map_err(|_| ResolutionError::SourceMaterial)?;
    let window_spec_id = material.window_spec().to_bytes();
    let statistic_spec_id = material.statistic_spec().to_bytes();
    drop(material_data);

    let spec_data = account(accounts, base + 2)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        account(accounts, base + 2)?,
        account(accounts, base + 3)?,
        SOURCE_SPEC_SCHEMA_ID_V1,
        request.source_spec,
        &spec_data,
        SOURCE_SPEC_BYTES,
    )?;
    let source = SourceSpecV1::decode(&spec_data).map_err(|_| ResolutionError::SourceMaterial)?;
    let provider_release_id = source.provider_release_id().to_bytes();
    let reference_id = source.adapter_config_id().to_bytes();
    drop(spec_data);
    if reference_id != request.parent_reference {
        return Err(ResolutionError::DerivedReference.into());
    }

    let provider_data = account(accounts, base + 4)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        account(accounts, base + 4)?,
        account(accounts, base + 5)?,
        PROVIDER_RELEASE_SCHEMA_ID_V1,
        provider_release_id,
        &provider_data,
        PROVIDER_RELEASE_BYTES,
    )?;
    let provider_release =
        ProviderReleaseV1::decode(&provider_data).map_err(|_| ResolutionError::ProviderRelease)?;
    drop(provider_data);

    let window_data = account(accounts, base + 6)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        account(accounts, base + 6)?,
        account(accounts, base + 7)?,
        WINDOW_SPEC_SCHEMA_ID_V1,
        window_spec_id,
        &window_data,
        WINDOW_SPEC_BYTES,
    )?;
    let window = WindowSpecV1::decode(&window_data).map_err(|_| ResolutionError::SourceMaterial)?;
    drop(window_data);

    let statistic_data = account(accounts, base + 8)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        account(accounts, base + 8)?,
        account(accounts, base + 9)?,
        STATISTIC_SPEC_SCHEMA_ID_V1,
        statistic_spec_id,
        &statistic_data,
        STATISTIC_SPEC_BYTES,
    )?;
    let statistic =
        StatisticSpecV1::decode(&statistic_data).map_err(|_| ResolutionError::SourceMaterial)?;
    drop(statistic_data);

    let reference_data = account(accounts, DERIVED_SETTLE_PARENT_REFERENCE_RAW_INDEX_V1)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        account(accounts, DERIVED_SETTLE_PARENT_REFERENCE_RAW_INDEX_V1)?,
        account(accounts, DERIVED_SETTLE_PARENT_REFERENCE_RAW_INDEX_V1 + 1)?,
        PARENT_REFERENCE_SCHEMA_ID_V1,
        reference_id,
        &reference_data,
        PARENT_REFERENCE_BYTES_V1,
    )
    .map_err(|_| ResolutionError::DerivedReference)?;
    let reference = ParentReferenceV1::decode(&reference_data)
        .map_err(|_| ResolutionError::DerivedReference)?;
    drop(reference_data);

    let id =
        |value: [u8; 32]| SourceContentId::new(value).map_err(|_| ResolutionError::SourceMaterial);
    Ok(AuthenticatedDerivedSourceRecordsV1 {
        material_id: id(request.source_material)?,
        material,
        source_spec_id: id(request.source_spec)?,
        source,
        provider_release_id: id(provider_release_id)?,
        provider_release,
        window_spec_id: id(window_spec_id)?,
        window,
        statistic_spec_id: id(statistic_spec_id)?,
        statistic,
        reference_id: id(reference_id)?,
        reference,
    })
}

fn boxed_parent(
    program_id: &Pubkey,
    core_program: &Pubkey,
    accounts: &[AccountInfo<'_>],
    base: usize,
    ordinary_count: u32,
) -> Result<Box<AuthenticatedParentCertificateV1>, ProgramError> {
    Ok(Box::new(authenticate_parent(
        program_id,
        core_program,
        accounts,
        base,
        ordinary_count,
    )?))
}

/// One parent's three accounts, in the emitted order: Market, Source state,
/// certificate.
///
/// The Market is Core's and at its own derived address; the Source state is
/// this program's, at its own derived address, bound to that Market and to
/// the Market's own resolution policy; the certificate is at the address the
/// Source state, the kind the state's phase implies, and the state's own
/// terminal sequence derive -- the seat Core's `AdmitTerminal` would read.
/// The parent's phase decides `terminal`: `Resolved` and `FailureCommitted`
/// are the two terminals; anything else is presented as not terminal and the
/// seam refuses it by name rather than this outer refusing on a missing seat.
#[inline(never)]
fn authenticate_parent(
    program_id: &Pubkey,
    core_program: &Pubkey,
    accounts: &[AccountInfo<'_>],
    base: usize,
    ordinary_count: u32,
) -> Result<AuthenticatedParentCertificateV1, ProgramError> {
    let market_account = account(accounts, base)?;
    let state_account = account(accounts, base + 1)?;
    let certificate_account = account(accounts, base + 2)?;
    if market_account.owner != core_program || market_account.executable {
        return Err(ResolutionError::DerivedWrongParent.into());
    }
    let market_data = market_account
        .try_borrow_data()
        .map_err(|_| ResolutionError::DerivedWrongParent)?;
    let market =
        CoreState::decode(&market_data).map_err(|_| ResolutionError::DerivedWrongParent)?;
    if Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(market.identity).as_slices(),
        core_program,
    )
    .0 != *market_account.key
    {
        return Err(ResolutionError::DerivedWrongParent.into());
    }
    let generation = market.identity.generation;
    let product_record_digest = market.identity.product_record.to_bytes();
    let material_id = market.identity.resolution_policy;
    drop(market_data);

    authenticate_source_state_account(program_id, state_account, market_account)?;
    let state_data = state_account
        .try_borrow_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let state =
        SourceResolutionStateV2::decode(&state_data).map_err(|_| ResolutionError::OutputState)?;
    if state.generation() != generation || state.material_id().to_bytes() != material_id.to_bytes()
    {
        return Err(ResolutionError::DerivedWrongParent.into());
    }
    let (kind, kind_tag) = match state.phase() {
        SourceResolutionPhaseV1::Resolved => (ResolutionCertificateKindV2::ResolutionSuccess, 1_u8),
        SourceResolutionPhaseV1::FailureCommitted => {
            (ResolutionCertificateKindV2::ResolutionFailure, 4_u8)
        }
        SourceResolutionPhaseV1::Primary
        | SourceResolutionPhaseV1::Recovery
        | SourceResolutionPhaseV1::Exhausted
        | SourceResolutionPhaseV1::Retired => {
            // Not terminal, or terminal and already retired with its seat
            // possibly reclaimed (design §4.4): presented as nothing.
            return Ok(AuthenticatedParentCertificateV1 {
                certificate: ParentCertificateV1 {
                    market: market_account.key.to_bytes(),
                    generation,
                    product_record_digest,
                    ordinary_count,
                    selector: 0,
                    terminal: false,
                },
                observed_at: 0,
                bytes: [0; RESOLUTION_CERTIFICATE_BYTES_V2],
            });
        }
    };
    let projection = state
        .terminal_projection()
        .map_err(|_| ResolutionError::OutputState)?;
    let sequence = projection.terminal_sequence().to_le_bytes();
    drop(state_data);

    let expected = Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            state_account.key.as_ref(),
            &[kind_tag],
            &sequence,
        ],
        program_id,
    )
    .0;
    if certificate_account.key != &expected
        || certificate_account.owner != program_id
        || certificate_account.executable
        || certificate_account.data_len() != RESOLUTION_CERTIFICATE_BYTES_V2
        || !funded_rent_persists_v1(certificate_account.lamports())
    {
        return Err(ResolutionError::DerivedParentNotTerminal.into());
    }
    let certificate_data = certificate_account
        .try_borrow_data()
        .map_err(|_| ResolutionError::DerivedParentNotTerminal)?;
    let certificate = ResolutionCertificateV2::decode(&certificate_data)
        .map_err(|_| ResolutionError::DerivedParentNotTerminal)?;
    if certificate.kind != kind
        || certificate.market != market_account.key.to_bytes()
        || certificate.generation != generation
        || certificate.product_record_digest != product_record_digest
        || certificate.receipt_account != certificate_account.key.to_bytes()
        || certificate.selector != projection.selector()
    {
        return Err(ResolutionError::DerivedParentNotTerminal.into());
    }
    let width = ordinary_count
        .checked_add(1)
        .ok_or(ResolutionError::Arithmetic)?;
    // `parentWidthMismatch`: the reference's ordinary count, proved against
    // the parent's own domain at founding, must be the width this certificate
    // was admitted under.
    certificate
        .validate_terminal_product(product_record_digest, width)
        .map_err(|_| ResolutionError::DerivedParentWidth)?;
    let mut bytes = [0_u8; RESOLUTION_CERTIFICATE_BYTES_V2];
    bytes.copy_from_slice(&certificate_data);
    Ok(AuthenticatedParentCertificateV1 {
        certificate: ParentCertificateV1 {
            market: certificate.market,
            generation: certificate.generation,
            product_record_digest: certificate.product_record_digest,
            ordinary_count,
            selector: certificate.selector,
            terminal: true,
        },
        observed_at: certificate.observed_at,
        bytes,
    })
}

/// Seat the child's certificate and write both outputs, commit-last.
#[allow(clippy::too_many_arguments)]
fn commit_settlement<'info>(
    program_id: &Pubkey,
    terminal_sequence: u64,
    source_state: &AccountInfo<'info>,
    certificate: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    rent: &Rent,
    next_source: &[u8; SOURCE_RESOLUTION_STATE_BYTES_V2],
    next_certificate: &[u8; RESOLUTION_CERTIFICATE_BYTES_V2],
) -> ProgramResult {
    initialize_certificate_at_kind(
        program_id,
        crate::relay_transport_v1::RESOLUTION_SUCCESS_CERTIFICATE_KIND_SEED,
        terminal_sequence,
        source_state,
        certificate,
        system,
        rent,
    )?;
    let mut state_output = source_state
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut certificate_output = certificate
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    if state_output.len() != SOURCE_RESOLUTION_STATE_BYTES_V2
        || certificate_output.len() != RESOLUTION_CERTIFICATE_BYTES_V2
        || certificate_output.iter().any(|byte| *byte != 0)
    {
        return Err(ResolutionError::OutputState.into());
    }
    state_output.copy_from_slice(next_source);
    certificate_output.copy_from_slice(next_certificate);
    Ok(())
}
