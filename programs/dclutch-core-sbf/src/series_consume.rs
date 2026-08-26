//! Release-selected Trading-to-Core recurring-Series Consume admission.
//!
//! This first stage authenticates every authority and Series prestate, creates
//! the exact Founding/Prepaid Market, and returns a producer-bound
//! acknowledgement. Custody realization, Claims founding, and final Core Open
//! are separate typed dependency stages owned by the common Hot controller.

use alloc::boxed::Box;
use core::cmp::min;

use dclutch_capability_contract::{
    CapabilityFundingDerivationV1, CapabilityManifestV1, ContentId as CapabilityContentId,
    FUNDING_STATE_BYTES, FundingCustodyObservationV1, FundingStateV1, FundingStatus,
};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
};
use dclutch_claims_svm::{
    founding_v5::{
        ClaimsFoundingAggregateSeedsV5, ClaimsFoundingRequestInputV5, ClaimsFoundingRequestV5,
    },
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        liability_basis_vector_width_v2,
    },
    protocol_position_v2::{
        PROTOCOL_POSITION_ADMISSION_BYTES_V2, ProtocolPositionAdmissionSeedsV2,
        ProtocolPositionSeedsV2,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    PROJECTED_CUSTODY_STATE_BYTES_V1, PROJECTED_HOARD_CONTEXT_DOMAIN_V1,
    ProjectedCustodyLockReceiptV1, ProjectedCustodyOperationV1, ProjectedCustodyPhaseV1,
    ProjectedCustodyStateSeedsV1, ProjectedCustodyStateV1,
};
use dclutch_market_core_codec::{
    Action, FoundingIntentV5, Request, Role, SERIES_FOUNDING_PERMIT_BYTES_V1, SeriesCoreActionV1,
    SERIES_FOUND_POST_RESOURCE_DIGEST_DOMAIN_V1, SeriesCoreFoundAckV2, SeriesCoreRequestV1,
    SeriesFoundingPermitSeedsV1, SeriesFoundingPermitV1,
};
use dclutch_product_runtime_v2_svm_reader::{
    FinalizedRecordFrameV2, authenticate_product_basis_v3,
};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_rent_contract::lifecycle_v2::{LIFECYCLE_RENT_CREDIT_BYTES_V2, LifecycleRentCreditV2};
use dclutch_series_v3_kernel::{
    AccountKeyV3, AuthenticatedProductProjectionV2, SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3,
    SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3, SERIES_TICKET_SCHEMA_RELEASE_ID_V3,
    admit_occurrence_bytes, admit_ticket, funding_list_id, future_market_projection,
    replay::{
        SERIES_STATE_BYTES_V3, SERIES_TICKET_STATE_BYTES_V3, SeriesStateV3, TicketPhaseV3,
        TicketStateSeedsV3, TicketStateV3,
    },
    template_content_id, ticket_content_id,
};
use dclutch_token_svm::{AccountState, TokenAccount, TokenProgram};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    hash::{hash, hashv},
    program::{invoke_signed, set_return_data},
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::{allocate, assign};

use crate::{
    CoreSbfError,
    found::{self, PreparedFound},
    frame::{FOUND_ACCOUNT_COUNT_V2, FoundAccounts, require_distinct},
    records::authenticate_finalized_record,
    release::{RoleBatchAdmissions, RoleDeploymentAccounts, authenticate_roles, identity},
};

/// Fixed account count before the ordered FundingState prefix and Claims suffix.
pub const SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V1: usize = FOUND_ACCOUNT_COUNT_V2 + 11;
/// Exact evidence suffix after the ordered FundingState accounts for Found.
pub const SERIES_CONSUME_FOUND_SUFFIX_ACCOUNT_COUNT_V1: usize = 15;
const MAXIMUM_FUNDING_STATES_V1: usize = 16;

struct SeriesConsumeAccounts<'accounts, 'info> {
    found: FoundAccounts<'accounts, 'info>,
    trading_program: &'accounts AccountInfo<'info>,
    trading_programdata: &'accounts AccountInfo<'info>,
    root: &'accounts AccountInfo<'info>,
    ticket_state: &'accounts AccountInfo<'info>,
    template_raw: &'accounts AccountInfo<'info>,
    template_staging: &'accounts AccountInfo<'info>,
    occurrence_raw: &'accounts AccountInfo<'info>,
    occurrence_staging: &'accounts AccountInfo<'info>,
    ticket_raw: &'accounts AccountInfo<'info>,
    ticket_staging: &'accounts AccountInfo<'info>,
    clock: &'accounts AccountInfo<'info>,
    tail: &'accounts [AccountInfo<'info>],
}

#[derive(Clone, Copy)]
struct SeriesFoundSuffix<'accounts, 'info> {
    permit: &'accounts AccountInfo<'info>,
    projected_replay: &'accounts AccountInfo<'info>,
    hoard: &'accounts AccountInfo<'info>,
    funding_source: &'accounts AccountInfo<'info>,
    funding_source_replay: &'accounts AccountInfo<'info>,
    linked_basis_raw: &'accounts AccountInfo<'info>,
    linked_basis_staging: &'accounts AccountInfo<'info>,
    claims_program: &'accounts AccountInfo<'info>,
    claims_programdata: &'accounts AccountInfo<'info>,
    custody_program: &'accounts AccountInfo<'info>,
    custody_programdata: &'accounts AccountInfo<'info>,
    aggregate: &'accounts AccountInfo<'info>,
    position: &'accounts AccountInfo<'info>,
    admission: &'accounts AccountInfo<'info>,
    founder: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> SeriesFoundSuffix<'accounts, 'info> {
    fn parse(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, CoreSbfError> {
        if accounts.len() != SERIES_CONSUME_FOUND_SUFFIX_ACCOUNT_COUNT_V1 {
            return Err(CoreSbfError::AccountFrame);
        }
        let value = Self {
            permit: account(accounts, 0)?,
            projected_replay: account(accounts, 1)?,
            hoard: account(accounts, 2)?,
            funding_source: account(accounts, 3)?,
            funding_source_replay: account(accounts, 4)?,
            linked_basis_raw: account(accounts, 5)?,
            linked_basis_staging: account(accounts, 6)?,
            claims_program: account(accounts, 7)?,
            claims_programdata: account(accounts, 8)?,
            custody_program: account(accounts, 9)?,
            custody_programdata: account(accounts, 10)?,
            aggregate: account(accounts, 11)?,
            position: account(accounts, 12)?,
            admission: account(accounts, 13)?,
            founder: account(accounts, 14)?,
        };
        if value.permit.is_signer
            || !value.permit.is_writable
            || value.permit.executable
            || value.claims_program.is_signer
            || value.claims_program.is_writable
            || !value.claims_program.executable
            || value.custody_program.is_signer
            || value.custody_program.is_writable
            || !value.custody_program.executable
        {
            return Err(CoreSbfError::AccountFrame);
        }
        for readonly in [
            value.projected_replay,
            value.hoard,
            value.funding_source,
            value.funding_source_replay,
            value.linked_basis_raw,
            value.linked_basis_staging,
            value.claims_programdata,
            value.custody_programdata,
            value.aggregate,
            value.position,
            value.admission,
            value.founder,
        ] {
            if readonly.is_signer || readonly.is_writable || readonly.executable {
                return Err(CoreSbfError::AccountFrame);
            }
        }
        Ok(value)
    }
}

impl<'accounts, 'info> SeriesConsumeAccounts<'accounts, 'info> {
    #[inline(never)]
    fn parse(
        program_id: &Pubkey,
        accounts: &'accounts [AccountInfo<'info>],
    ) -> Result<Self, CoreSbfError> {
        if accounts.len() <= SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V1 {
            return Err(CoreSbfError::AccountFrame);
        }
        let found_slice = accounts
            .get(..FOUND_ACCOUNT_COUNT_V2)
            .ok_or(CoreSbfError::AccountFrame)?;
        let fixed = accounts
            .get(..SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V1)
            .ok_or(CoreSbfError::AccountFrame)?;
        require_distinct(fixed)?;
        let found = FoundAccounts::parse(program_id, found_slice)?;
        let trading_program = account(accounts, 31)?;
        let trading_programdata = account(accounts, 32)?;
        let root = account(accounts, 33)?;
        let ticket_state = account(accounts, 34)?;
        let template_raw = account(accounts, 35)?;
        let template_staging = account(accounts, 36)?;
        let occurrence_raw = account(accounts, 37)?;
        let occurrence_staging = account(accounts, 38)?;
        let ticket_raw = account(accounts, 39)?;
        let ticket_staging = account(accounts, 40)?;
        let clock = account(accounts, 41)?;
        if trading_program.is_signer
            || trading_program.is_writable
            || !trading_program.executable
            || trading_programdata.is_signer
            || trading_programdata.is_writable
            || trading_programdata.executable
            || root.is_signer
            || root.is_writable
            || root.executable
            || ticket_state.is_signer
            || ticket_state.is_writable
            || ticket_state.executable
            || clock.key != &sysvar::clock::ID
            || clock.is_signer
            || clock.is_writable
            || clock.executable
        {
            return Err(CoreSbfError::AccountFrame);
        }
        for record in [
            template_raw,
            template_staging,
            occurrence_raw,
            occurrence_staging,
            ticket_raw,
            ticket_staging,
        ] {
            if record.is_signer || record.is_writable || record.executable {
                return Err(CoreSbfError::AccountFrame);
            }
        }
        Ok(Self {
            found,
            trading_program,
            trading_programdata,
            root,
            ticket_state,
            template_raw,
            template_staging,
            occurrence_raw,
            occurrence_staging,
            ticket_raw,
            ticket_staging,
            clock,
            tail: accounts
                .get(SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V1..)
                .ok_or(CoreSbfError::AccountFrame)?,
        })
    }
}

#[derive(Clone, Copy)]
struct AdmittedSeries {
    occurrence: dclutch_series_v3_kernel::AdmittedOccurrenceV3,
    ticket: dclutch_series_v3_kernel::AdmittedTicketV3,
    product: AuthenticatedProductProjectionV2,
}

struct PermitPlan {
    permit: Box<SeriesFoundingPermitV1>,
}

#[derive(Clone, Copy)]
struct ProductFacts {
    linked_basis_record: [u8; 32],
    semantic_basis: [u8; 32],
    claim_count: u32,
    basis_scale: u64,
}

#[derive(Clone, Copy)]
struct ProjectedFacts {
    realize_request_digest: [u8; 32],
    realize_receipt_digest: [u8; 32],
    realize_revision: u64,
    quantity: u64,
    expiry_slot: u64,
    hoard_amount: u64,
}

#[derive(Clone, Copy)]
struct FundingSpanEvidence {
    count: u8,
    list_id: [u8; 32],
}

/// Authenticate and create the Founding Core Market for one Series Consume.
#[inline(never)]
pub(crate) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: SeriesCoreRequestV1,
    request_bytes: &[u8],
    proof_bytes: &[u8],
    lock_receipt_bytes: &[u8],
) -> Result<(), solana_program::program_error::ProgramError> {
    if request.action() != SeriesCoreActionV1::Consume {
        return Err(CoreSbfError::Instruction.into());
    }
    let frame = SeriesConsumeAccounts::parse(program_id, accounts)?;
    require_distinct(accounts)?;
    let lock_receipt = decode_lock_receipt(lock_receipt_bytes)?;
    let rent = Rent::from_account_info(frame.found.rent).map_err(|_| CoreSbfError::Creation)?;
    let release_admissions = Box::new(authenticate_release_batch(&frame, request)?);

    // This single preparation establishes the infrastructure root, immutable
    // Core release, Registry records, Runtime Product, and vacant Market plan.
    // No Market bytes are written until every Series precondition also joins.
    let prepared = prepare_found(
        program_id,
        &frame.found,
        request,
        &rent,
        &release_admissions,
    )?;
    release_admissions.require(Role::Trading)?;
    authenticate_trading_caller(&frame, request, request_bytes)?;

    let admitted = authenticate_series(&frame, request, proof_bytes, &rent, program_id, &prepared)?;
    authenticate_root_and_replay(&frame, request, &admitted, program_id)?;
    let (funding_span, suffix_accounts) =
        split_funding_prefix(&frame, &admitted, request, &rent, &prepared)?;
    let suffix = SeriesFoundSuffix::parse(suffix_accounts)?;
    release_admissions.require(Role::Claims)?;
    release_admissions.require(Role::Custody)?;
    authenticate_found_coordinates(&frame, request, &admitted, &rent, &prepared)?;
    let permit_plan = prepare_permit(
        program_id,
        &frame,
        suffix,
        request,
        &admitted,
        &prepared,
        &lock_receipt,
        lock_receipt_bytes,
        &rent,
    )?;

    found::apply_prepared(program_id, &frame.found, *prepared)?;
    create_permit(
        program_id,
        suffix,
        frame.found.system,
        permit_plan.permit.as_ref(),
        &rent,
    )?;
    let market_bytes = frame
        .found
        .market
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Commit)?;
    let permit_bytes = suffix
        .permit
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Commit)?;
    let post_resource_digest = hashv(&[
        SERIES_FOUND_POST_RESOURCE_DIGEST_DOMAIN_V1,
        &market_bytes,
        &permit_bytes,
    ])
    .to_bytes();
    drop(market_bytes);
    drop(permit_bytes);
    let acknowledgement = SeriesCoreFoundAckV2::new(
        request,
        identity(program_id.to_bytes())?,
        identity(suffix.permit.key.to_bytes())?,
        identity(hash(request_bytes).to_bytes())?,
        funding_span.count,
        identity(funding_span.list_id)?,
        identity(post_resource_digest)?,
    )
    .map_err(|_| CoreSbfError::ChildAck)?;
    let bytes = acknowledgement
        .encode()
        .map_err(|_| CoreSbfError::ChildAck)?;
    set_return_data(&bytes);
    Ok(())
}

#[inline(never)]
fn decode_lock_receipt(input: &[u8]) -> Result<Box<ProjectedCustodyLockReceiptV1>, CoreSbfError> {
    Ok(Box::new(
        ProjectedCustodyLockReceiptV1::decode(input).map_err(|_| CoreSbfError::ChildAck)?,
    ))
}

#[inline(never)]
fn prepare_found(
    program_id: &Pubkey,
    frame: &FoundAccounts<'_, '_>,
    request: SeriesCoreRequestV1,
    rent: &Rent,
    release_admissions: &RoleBatchAdmissions,
) -> Result<Box<PreparedFound>, solana_program::program_error::ProgramError> {
    let found_request = Request::administrative(
        Action::Found,
        request
            .market_generation()
            .ok_or(CoreSbfError::Instruction)?,
        identity(
            request
                .market()
                .ok_or(CoreSbfError::Instruction)?
                .to_bytes(),
        )?,
    );
    Ok(Box::new(found::prepare_with_admission(
        program_id,
        frame,
        found_request,
        rent,
        release_admissions.admission(Role::Core)?,
    )?))
}

#[inline(never)]
fn fixed_suffix<'accounts, 'info>(
    frame: &'accounts SeriesConsumeAccounts<'accounts, 'info>,
) -> Result<SeriesFoundSuffix<'accounts, 'info>, CoreSbfError> {
    let funding_count = frame
        .tail
        .len()
        .checked_sub(SERIES_CONSUME_FOUND_SUFFIX_ACCOUNT_COUNT_V1)
        .ok_or(CoreSbfError::AccountFrame)?;
    if funding_count == 0 || funding_count > MAXIMUM_FUNDING_STATES_V1 {
        return Err(CoreSbfError::AccountFrame);
    }
    SeriesFoundSuffix::parse(
        frame
            .tail
            .get(funding_count..)
            .ok_or(CoreSbfError::AccountFrame)?,
    )
}

#[inline(never)]
fn authenticate_release_batch<'accounts, 'info>(
    frame: &SeriesConsumeAccounts<'accounts, 'info>,
    request: SeriesCoreRequestV1,
) -> Result<RoleBatchAdmissions, CoreSbfError> {
    let suffix = fixed_suffix(frame)?;
    let requested = [
        RoleDeploymentAccounts::new(
            Role::Core,
            frame.found.core_program,
            frame.found.core_programdata,
        ),
        RoleDeploymentAccounts::new(
            Role::Claims,
            suffix.claims_program,
            suffix.claims_programdata,
        ),
        RoleDeploymentAccounts::new(
            Role::Trading,
            frame.trading_program,
            frame.trading_programdata,
        ),
        RoleDeploymentAccounts::new(
            Role::Custody,
            suffix.custody_program,
            suffix.custody_programdata,
        ),
    ];
    authenticate_roles(
        frame.found.activation_cache,
        frame.found.registry_program,
        identity(frame.found.registry_program.key.to_bytes())?,
        request.release_set().to_bytes(),
        &requested,
    )
}

#[inline(never)]
fn authenticate_trading_caller(
    frame: &SeriesConsumeAccounts<'_, '_>,
    request: SeriesCoreRequestV1,
    request_bytes: &[u8],
) -> Result<(), CoreSbfError> {
    let market = request
        .market()
        .ok_or(CoreSbfError::Instruction)?
        .to_bytes();
    let request_digest = hash(request_bytes).to_bytes();
    let ticket_context = ticket_content_id_from_account(frame.ticket_raw)?;
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        request.release_set().to_bytes(),
        market,
        ExecutionRoleV1::Trading,
        ticket_context,
        request_digest,
    )
    .map_err(|_| CoreSbfError::CallerAuthority)?;
    let expected = Pubkey::find_program_address(&seeds.as_slices(), frame.trading_program.key).0;
    if frame.found.payer.key != &expected || !frame.found.payer.is_signer {
        return Err(CoreSbfError::CallerAuthority);
    }
    Ok(())
}

#[inline(never)]
fn authenticate_series(
    frame: &SeriesConsumeAccounts<'_, '_>,
    request: SeriesCoreRequestV1,
    proof_bytes: &[u8],
    rent: &Rent,
    program_id: &Pubkey,
    prepared: &PreparedFound,
) -> Result<Box<AdmittedSeries>, CoreSbfError> {
    let template_bytes = frame
        .template_raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    authenticate_series_record(
        frame,
        frame.template_raw,
        frame.template_staging,
        rent,
        SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
        &template_bytes,
    )?;
    let occurrence_bytes = frame
        .occurrence_raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    authenticate_series_record(
        frame,
        frame.occurrence_raw,
        frame.occurrence_staging,
        rent,
        SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3,
        &occurrence_bytes,
    )?;
    let ticket_bytes = frame
        .ticket_raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    authenticate_series_record(
        frame,
        frame.ticket_raw,
        frame.ticket_staging,
        rent,
        SERIES_TICKET_SCHEMA_RELEASE_ID_V3,
        &ticket_bytes,
    )?;

    let occurrence = admit_occurrence_bytes(&template_bytes, &occurrence_bytes, proof_bytes)
        .map_err(|_| CoreSbfError::Reference)?;
    let ticket = admit_ticket(&ticket_bytes).map_err(|_| CoreSbfError::Reference)?;
    occurrence
        .require_ticket(ticket.ticket())
        .map_err(|_| CoreSbfError::Reference)?;
    if template_content_id(&template_bytes)
        .map_err(|_| CoreSbfError::Reference)?
        .to_bytes()
        != request.template().to_bytes()
        || ticket.content_id().to_bytes() != ticket_content_id_from_account(frame.ticket_raw)?
    {
        return Err(CoreSbfError::Reference);
    }

    let product = AuthenticatedProductProjectionV2::new(
        ContentId::new(prepared.product.product_record.to_bytes())
            .map_err(|_| CoreSbfError::Reference)?,
        ContentId::new(prepared.product.product_id.to_bytes())
            .map_err(|_| CoreSbfError::Reference)?,
        ContentId::new(prepared.product.result_domain.to_bytes())
            .map_err(|_| CoreSbfError::Reference)?,
    );
    if request
        .product()
        .ok_or(CoreSbfError::Instruction)?
        .to_bytes()
        != product.product_record().to_bytes()
    {
        return Err(CoreSbfError::Reference);
    }
    let future = future_market_projection(
        occurrence,
        product,
        AccountKeyV3::new(frame.found.registry_program.key.to_bytes())
            .map_err(|_| CoreSbfError::Reference)?,
    )
    .map_err(|_| CoreSbfError::Reference)?;
    let expected = Pubkey::find_program_address(&future.seeds().as_slices(), program_id).0;
    future
        .require_address(
            AccountKeyV3::new(expected.to_bytes()).map_err(|_| CoreSbfError::Reference)?,
        )
        .map_err(|_| CoreSbfError::Reference)?;
    if frame.found.market.key != &expected {
        return Err(CoreSbfError::Market);
    }
    Ok(Box::new(AdmittedSeries {
        occurrence,
        ticket,
        product,
    }))
}

fn authenticate_series_record(
    frame: &SeriesConsumeAccounts<'_, '_>,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    rent: &Rent,
    schema: [u8; 32],
    bytes: &[u8],
) -> Result<(), CoreSbfError> {
    authenticate_finalized_record(
        frame.found.registry_program.key,
        raw,
        staging,
        rent,
        schema,
        hash(bytes).to_bytes(),
        bytes,
    )?;
    Ok(())
}

#[inline(never)]
fn authenticate_root_and_replay(
    frame: &SeriesConsumeAccounts<'_, '_>,
    request: SeriesCoreRequestV1,
    admitted: &AdmittedSeries,
    _program_id: &Pubkey,
) -> Result<(), CoreSbfError> {
    if frame.root.owner != frame.trading_program.key
        || frame.root.data_len() != CAPABILITY_ROOT_HEADER_BYTES_V1 + SERIES_STATE_BYTES_V3
        || frame.ticket_state.owner != frame.trading_program.key
        || frame.ticket_state.data_len() != SERIES_TICKET_STATE_BYTES_V3
    {
        return Err(CoreSbfError::Reference);
    }
    let root_bytes = frame
        .root
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Reference)?;
    let header = CapabilityRootHeaderV1::decode(
        root_bytes
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or(CoreSbfError::Reference)?,
    )
    .map_err(|_| CoreSbfError::Reference)?;
    let expected_root =
        Pubkey::find_program_address(&header.seeds().as_slices(), frame.trading_program.key).0;
    if frame.root.key != &expected_root
        || header.release_set().to_bytes() != request.release_set().to_bytes()
        || header.selection().config().to_bytes() != request.template().to_bytes()
    {
        return Err(CoreSbfError::Reference);
    }
    let series = SeriesStateV3::decode(
        root_bytes
            .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .ok_or(CoreSbfError::Reference)?,
        admitted.occurrence.template().occurrence_count(),
    )
    .map_err(|_| CoreSbfError::Reference)?;
    if series.next_occurrence() != admitted.occurrence.occurrence().occurrence() {
        return Err(CoreSbfError::Reference);
    }
    series
        .settle_current(
            request.expected_series_revision(),
            admitted.occurrence.template().occurrence_count(),
        )
        .map_err(|_| CoreSbfError::Reference)?;

    let ticket_bytes = frame
        .ticket_state
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Reference)?;
    let ticket_state = TicketStateV3::decode(&ticket_bytes).map_err(|_| CoreSbfError::Reference)?;
    let ticket_seeds =
        TicketStateSeedsV3::new(frame.root.key.to_bytes(), admitted.ticket.content_id());
    let expected_ticket =
        Pubkey::find_program_address(&ticket_seeds.as_slices(), frame.trading_program.key).0;
    if frame.ticket_state.key != &expected_ticket
        || request
            .ticket()
            .ok_or(CoreSbfError::Instruction)?
            .to_bytes()
            != frame.ticket_state.key.to_bytes()
        || ticket_state.ticket_record_id() != admitted.ticket.content_id()
    {
        return Err(CoreSbfError::Reference);
    }
    ticket_state
        .settle(request.expected_ticket_revision(), TicketPhaseV3::Consumed)
        .map_err(|_| CoreSbfError::Reference)?;
    Ok(())
}

#[inline(never)]
fn split_funding_prefix<'a, 'info>(
    frame: &'a SeriesConsumeAccounts<'a, 'info>,
    admitted: &AdmittedSeries,
    request: SeriesCoreRequestV1,
    rent: &Rent,
    prepared: &PreparedFound,
) -> Result<(FundingSpanEvidence, &'a [AccountInfo<'info>]), CoreSbfError> {
    let maximum = min(frame.tail.len(), MAXIMUM_FUNDING_STATES_V1);
    let placeholder = AccountKeyV3::new([1; 32]).map_err(|_| CoreSbfError::Funding)?;
    let mut keys = [placeholder; MAXIMUM_FUNDING_STATES_V1];
    let mut matched = None;
    for count in 1..=maximum {
        let account = frame
            .tail
            .get(count - 1)
            .ok_or(CoreSbfError::AccountFrame)?;
        *keys.get_mut(count - 1).ok_or(CoreSbfError::Arithmetic)? =
            AccountKeyV3::new(account.key.to_bytes()).map_err(|_| CoreSbfError::Funding)?;
        let list_id = funding_list_id(keys.get(..count).ok_or(CoreSbfError::Arithmetic)?)
            .map_err(|_| CoreSbfError::Funding)?;
        if list_id == admitted.ticket.ticket().funding_list() {
            matched = Some((count, list_id.to_bytes()));
            break;
        }
    }
    let (count, list_id) = matched.ok_or(CoreSbfError::Funding)?;
    let funding = frame.tail.get(..count).ok_or(CoreSbfError::AccountFrame)?;
    let claims = frame.tail.get(count..).ok_or(CoreSbfError::AccountFrame)?;
    authenticate_funding(frame, admitted, request, funding, rent, prepared)?;
    Ok((
        FundingSpanEvidence {
            count: u8::try_from(count).map_err(|_| CoreSbfError::Arithmetic)?,
            list_id,
        },
        claims,
    ))
}

fn authenticate_funding(
    frame: &SeriesConsumeAccounts<'_, '_>,
    admitted: &AdmittedSeries,
    request: SeriesCoreRequestV1,
    accounts: &[AccountInfo<'_>],
    rent: &Rent,
    prepared: &PreparedFound,
) -> Result<(), CoreSbfError> {
    let manifest_data = frame
        .found
        .manifest_raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    let manifest_id = prepared.manifest_id;
    if manifest_id
        != admitted
            .occurrence
            .occurrence()
            .capability_manifest()
            .to_bytes()
    {
        return Err(CoreSbfError::Funding);
    }
    let manifest =
        CapabilityManifestV1::decode(&manifest_data).map_err(|_| CoreSbfError::Funding)?;
    let manifest_id = CapabilityContentId::new(manifest_id).map_err(|_| CoreSbfError::Funding)?;
    let mut native_total = 0_u64;
    let mut previous = None;
    for account in accounts {
        if account.owner != frame.trading_program.key
            || account.data_len() != FUNDING_STATE_BYTES
            || account.is_signer
            || account.is_writable
            || account.executable
            || account.key == frame.root.key
            || account.key == frame.ticket_state.key
        {
            return Err(CoreSbfError::Funding);
        }
        let data = account
            .try_borrow_data()
            .map_err(|_| CoreSbfError::Funding)?;
        let funding = FundingStateV1::decode(&data).map_err(|_| CoreSbfError::Funding)?;
        if funding.status() != FundingStatus::Pending
            || funding.manifest_content_id() != manifest_id
            || previous.is_some_and(|entry| entry >= funding.entry_index())
        {
            return Err(CoreSbfError::Funding);
        }
        let custody = FundingCustodyObservationV1::native_only(
            account.lamports(),
            rent.minimum_balance(FUNDING_STATE_BYTES),
        )
        .map_err(|_| CoreSbfError::Funding)?;
        funding
            .validate_against(manifest_id, manifest, custody)
            .map_err(|_| CoreSbfError::Funding)?;
        let derivation = CapabilityFundingDerivationV1::new(
            frame.found.market.key.to_bytes(),
            request
                .market_generation()
                .ok_or(CoreSbfError::Instruction)?,
            manifest_id,
            manifest,
            funding,
        )
        .map_err(|_| CoreSbfError::Funding)?;
        if Pubkey::find_program_address(&derivation.seed_components(), frame.trading_program.key).0
            != *account.key
        {
            return Err(CoreSbfError::Funding);
        }
        native_total = native_total
            .checked_add(account.lamports())
            .ok_or(CoreSbfError::Arithmetic)?;
        previous = Some(funding.entry_index());
    }
    if native_total != request.capability_rent() {
        return Err(CoreSbfError::Funding);
    }
    Ok(())
}

#[inline(never)]
fn authenticate_found_coordinates(
    frame: &SeriesConsumeAccounts<'_, '_>,
    request: SeriesCoreRequestV1,
    admitted: &AdmittedSeries,
    rent: &Rent,
    prepared: &PreparedFound,
) -> Result<(), CoreSbfError> {
    let occurrence = admitted.occurrence.occurrence();
    let ticket = admitted.ticket.ticket();
    if request
        .market()
        .ok_or(CoreSbfError::Instruction)?
        .to_bytes()
        != occurrence.market().to_bytes()
        || request.release_set().to_bytes()
            != admitted.occurrence.template().release_set().to_bytes()
        || request.realm().ok_or(CoreSbfError::Instruction)?.to_bytes()
            != admitted.occurrence.template().realm().to_bytes()
        || request
            .product()
            .ok_or(CoreSbfError::Instruction)?
            .to_bytes()
            != admitted.product.product_record().to_bytes()
        || request
            .founder()
            .ok_or(CoreSbfError::Instruction)?
            .to_bytes()
            != ticket.founder().to_bytes()
        || request.beneficiary().to_bytes() != ticket.refund_owner().to_bytes()
        || request.occurrence_index() != occurrence.occurrence()
        || request.market_rent() != occurrence.funds().market_rent()
        || request.capability_rent() != occurrence.funds().capability_native()
        || request.work() != occurrence.funds().founding_work()
        || request.hoard_principal() != occurrence.funds().hoard_principal()
        || frame.found.market.lamports() != request.market_rent()
        || frame.found.market.owner != &system_program::ID
        || frame.found.market.data_len() != 0
    {
        return Err(CoreSbfError::Reference);
    }
    if prepared.realm_id != admitted.occurrence.template().realm().to_bytes()
        || prepared.resolution_policy_id != occurrence.resolution_policy().to_bytes()
        || prepared.release_set_id != request.release_set().to_bytes()
    {
        return Err(CoreSbfError::Reference);
    }
    let credit_data = frame
        .found
        .rent_credit
        .try_borrow_data()
        .map_err(|_| CoreSbfError::RentCredit)?;
    let credit =
        LifecycleRentCreditV2::decode(&credit_data).map_err(|_| CoreSbfError::RentCredit)?;
    if credit.refund_wallet().to_bytes() != request.beneficiary().to_bytes()
        || credit.market().to_bytes()
            != request
                .market()
                .ok_or(CoreSbfError::Instruction)?
                .to_bytes()
        || credit.release_set().to_bytes() != request.release_set().to_bytes()
        || credit.generation()
            != request
                .market_generation()
                .ok_or(CoreSbfError::Instruction)?
        || frame.found.rent_credit.data_len() != LIFECYCLE_RENT_CREDIT_BYTES_V2
        || !rent.is_exempt(
            frame.found.rent_credit.lamports(),
            LIFECYCLE_RENT_CREDIT_BYTES_V2,
        )
    {
        return Err(CoreSbfError::RentCredit);
    }
    let clock = Clock::from_account_info(frame.clock).map_err(|_| CoreSbfError::Creation)?;
    let retry_through = admitted
        .occurrence
        .template()
        .retry_through(occurrence.occurrence())
        .map_err(|_| CoreSbfError::Reference)?;
    if clock.slot < occurrence.scheduled_slot() || clock.slot > retry_through {
        return Err(CoreSbfError::Reference);
    }
    Ok(())
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn prepare_permit<'accounts, 'info>(
    program_id: &Pubkey,
    frame: &SeriesConsumeAccounts<'accounts, 'info>,
    suffix: SeriesFoundSuffix<'accounts, 'info>,
    request: SeriesCoreRequestV1,
    admitted: &AdmittedSeries,
    prepared: &PreparedFound,
    lock_receipt: &ProjectedCustodyLockReceiptV1,
    lock_receipt_bytes: &[u8],
    rent: &Rent,
) -> Result<PermitPlan, CoreSbfError> {
    let product = authenticate_product_facts(frame, suffix, prepared, rent)?;
    let projected = authenticate_projected_facts(
        program_id,
        frame,
        suffix,
        request,
        admitted,
        prepared,
        lock_receipt,
        product,
    )?;
    build_permit_plan(
        program_id,
        frame,
        suffix,
        request,
        admitted,
        prepared,
        lock_receipt,
        lock_receipt_bytes,
        product,
        projected,
        rent,
    )
}

#[inline(never)]
fn authenticate_product_facts<'accounts, 'info>(
    frame: &SeriesConsumeAccounts<'accounts, 'info>,
    suffix: SeriesFoundSuffix<'accounts, 'info>,
    prepared: &PreparedFound,
    rent: &Rent,
) -> Result<ProductFacts, CoreSbfError> {
    let product = authenticate_product_basis_v3(
        frame.found.registry_program.key,
        rent,
        *prepared.runtime,
        FinalizedRecordFrameV2 {
            raw: suffix.linked_basis_raw,
            staging: suffix.linked_basis_staging,
        },
    )
    .map_err(|_| CoreSbfError::Reference)?;
    if product.runtime.product_record.content_digest.to_bytes() != prepared.product_record_id
        || product.runtime.product_id.to_bytes() != prepared.product_id
        || product
            .runtime
            .result_domain_record
            .content_digest
            .to_bytes()
            != prepared.product.result_domain.to_bytes()
        || product.runtime.portfolio_record.content_digest.to_bytes()
            != prepared.product.portfolio.to_bytes()
        || product.runtime.outcome_count != prepared.product.outcome_count
        || product.semantic_basis_id.to_bytes() != prepared.product.liability_basis.to_bytes()
        || product.basis_width != product.runtime.outcome_count
        || product.payout_scale == 0
    {
        return Err(CoreSbfError::Reference);
    }
    Ok(ProductFacts {
        linked_basis_record: product.linked_basis_record.content_digest.to_bytes(),
        semantic_basis: product.semantic_basis_id.to_bytes(),
        claim_count: product.runtime.outcome_count,
        basis_scale: product.payout_scale,
    })
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn authenticate_projected_facts(
    program_id: &Pubkey,
    frame: &SeriesConsumeAccounts<'_, '_>,
    suffix: SeriesFoundSuffix<'_, '_>,
    request: SeriesCoreRequestV1,
    admitted: &AdmittedSeries,
    prepared: &PreparedFound,
    lock_receipt: &ProjectedCustodyLockReceiptV1,
    product: ProductFacts,
) -> Result<ProjectedFacts, CoreSbfError> {
    let projected = authenticate_projected_state(
        program_id,
        frame,
        suffix,
        request,
        admitted,
        prepared,
        lock_receipt,
    )?;
    let quantity = request
        .hoard_principal()
        .checked_div(product.basis_scale)
        .filter(|quantity| *quantity > 0)
        .ok_or(CoreSbfError::Arithmetic)?;
    if quantity
        .checked_mul(product.basis_scale)
        .ok_or(CoreSbfError::Arithmetic)?
        != request.hoard_principal()
    {
        return Err(CoreSbfError::Funding);
    }
    let realize_request_digest = realize_request_digest(&projected)?;
    let (realize_receipt_digest, realize_revision) = realize_receipt_facts(
        &projected,
        realize_request_digest,
        prepared.candidate_state(),
        request.hoard_principal(),
        frame.found.rent_credit.key.to_bytes(),
    )?;
    let expiry_slot = admitted
        .occurrence
        .template()
        .retry_through(admitted.occurrence.occurrence().occurrence())
        .map_err(|_| CoreSbfError::Reference)?;
    Ok(ProjectedFacts {
        realize_request_digest,
        realize_receipt_digest,
        realize_revision,
        quantity,
        expiry_slot,
        hoard_amount: request.hoard_principal(),
    })
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn authenticate_projected_state(
    program_id: &Pubkey,
    frame: &SeriesConsumeAccounts<'_, '_>,
    suffix: SeriesFoundSuffix<'_, '_>,
    request: SeriesCoreRequestV1,
    admitted: &AdmittedSeries,
    prepared: &PreparedFound,
    lock_receipt: &ProjectedCustodyLockReceiptV1,
) -> Result<Box<ProjectedCustodyStateV1>, CoreSbfError> {
    let data = suffix
        .projected_replay
        .try_borrow_data()
        .map_err(|_| CoreSbfError::ChildAck)?;
    if suffix.projected_replay.owner != suffix.custody_program.key
        || data.len() != PROJECTED_CUSTODY_STATE_BYTES_V1
    {
        return Err(CoreSbfError::ChildAck);
    }
    let projected =
        Box::new(ProjectedCustodyStateV1::decode(&data).map_err(|_| CoreSbfError::ChildAck)?);
    drop(data);
    let seeds = ProjectedCustodyStateSeedsV1::from_request(projected.request);
    let (expected, bump) =
        Pubkey::find_program_address(&seeds.as_slices(), suffix.custody_program.key);
    let ticket_context = admitted.ticket.content_id().to_bytes();
    let context = hashv(&[PROJECTED_HOARD_CONTEXT_DOMAIN_V1, ticket_context.as_slice()]).to_bytes();
    let expiry = admitted
        .occurrence
        .template()
        .retry_through(admitted.occurrence.occurrence().occurrence())
        .map_err(|_| CoreSbfError::Reference)?;
    if expected != *suffix.projected_replay.key
        || bump != projected.bump
        || projected.phase != ProjectedCustodyPhaseV1::HoardLocked
        || projected.request.market != frame.found.market.key.to_bytes()
        || projected.request.generation
            != request
                .market_generation()
                .ok_or(CoreSbfError::Instruction)?
        || projected.request.realm != prepared.realm_id
        || projected.request.product_record != prepared.product_record_id
        || projected.request.product != prepared.product_id
        || projected.request.source != prepared.resolution_policy_id
        || projected.request.release_set != request.release_set().to_bytes()
        || projected.request.parent_capability_root != frame.root.key.to_bytes()
        || projected.request.context_digest != context
        || projected.request.caller_program != frame.trading_program.key.to_bytes()
        || projected.request.core_program != program_id.to_bytes()
        || projected.request.rent_program != frame.found.rent_program.key.to_bytes()
        || projected.request.refund_owner != request.beneficiary().to_bytes()
        || projected.request.rent_credit != frame.found.rent_credit.key.to_bytes()
        || projected.request.hoard_vault != suffix.hoard.key.to_bytes()
        || projected.request.funding_source_vault != suffix.funding_source.key.to_bytes()
        || projected.request.funding_source_context != ticket_context
        || projected.request.mint != prepared.collateral_mint
        || projected.request.token_program != prepared.token_program
        || projected.request.collateral_release != prepared.collateral_release
        || projected.request.expiry_slot != expiry
        || projected.locked_amount != request.hoard_principal()
        || projected.last_request_digest != lock_receipt.request_digest
        || projected.next_revision != lock_receipt.resulting_revision
        || lock_receipt.market != frame.found.market.key.to_bytes()
        || lock_receipt.release_set != request.release_set().to_bytes()
        || lock_receipt.context_digest != context
        || lock_receipt.source_vault != suffix.funding_source.key.to_bytes()
        || lock_receipt.source_replay != suffix.funding_source_replay.key.to_bytes()
        || lock_receipt.hoard_vault != suffix.hoard.key.to_bytes()
        || lock_receipt.rent_credit != frame.found.rent_credit.key.to_bytes()
        || lock_receipt.amount != request.hoard_principal()
        || lock_receipt.resulting_revision != projected.next_revision
    {
        return Err(CoreSbfError::ChildAck);
    }
    for closed in [suffix.funding_source, suffix.funding_source_replay] {
        if closed.owner != &system_program::ID
            || closed.lamports() != 0
            || !closed.data_is_empty()
            || closed.executable
        {
            return Err(CoreSbfError::ChildAck);
        }
    }
    if TokenProgram::parse(suffix.hoard.owner.to_bytes())
        .map_err(|_| CoreSbfError::ChildAck)?
        .program_id()
        != prepared.token_program
    {
        return Err(CoreSbfError::ChildAck);
    }
    let hoard_data = suffix
        .hoard
        .try_borrow_data()
        .map_err(|_| CoreSbfError::ChildAck)?;
    let hoard = TokenAccount::parse(&hoard_data).map_err(|_| CoreSbfError::ChildAck)?;
    if hoard.mint != prepared.collateral_mint
        || hoard.amount != request.hoard_principal()
        || hoard.state != AccountState::Initialized
        || !hoard.delegate.is_none()
        || hoard.delegated_amount != 0
        || !hoard.native_reserve.is_none()
        || !hoard.close_authority.is_none()
    {
        return Err(CoreSbfError::ChildAck);
    }
    Ok(projected)
}

fn canonical_realize_request(
    projected: &ProjectedCustodyStateV1,
) -> Result<dclutch_custody_contract::ProjectedCustodyRequestV1, CoreSbfError> {
    let mut request = projected.request;
    request.operation = ProjectedCustodyOperationV1::RealizeAndClose;
    request.expected_revision = projected.next_revision;
    request.resulting_revision = projected
        .next_revision
        .checked_add(1)
        .ok_or(CoreSbfError::Arithmetic)?;
    request.amount = projected.locked_amount;
    Ok(request)
}

#[inline(never)]
fn realize_request_digest(projected: &ProjectedCustodyStateV1) -> Result<[u8; 32], CoreSbfError> {
    Ok(hash(
        &canonical_realize_request(projected)?
            .encode()
            .map_err(|_| CoreSbfError::ChildAck)?,
    )
    .to_bytes())
}

#[inline(never)]
fn realize_receipt_facts(
    projected: &ProjectedCustodyStateV1,
    request_digest: [u8; 32],
    market: dclutch_market_core_codec::CoreState,
    hoard_amount: u64,
    rent_credit: [u8; 32],
) -> Result<([u8; 32], u64), CoreSbfError> {
    let market_digest = hash(&market.encode().map_err(|_| CoreSbfError::Transition)?).to_bytes();
    let receipt = projected
        .realize_and_close(
            canonical_realize_request(projected)?,
            request_digest,
            market,
            market_digest,
            hoard_amount,
            rent_credit,
        )
        .map_err(|_| CoreSbfError::ChildAck)?;
    Ok((
        hash(&receipt.encode().map_err(|_| CoreSbfError::ChildAck)?).to_bytes(),
        receipt.resulting_revision,
    ))
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn build_permit_plan(
    program_id: &Pubkey,
    frame: &SeriesConsumeAccounts<'_, '_>,
    suffix: SeriesFoundSuffix<'_, '_>,
    request: SeriesCoreRequestV1,
    admitted: &AdmittedSeries,
    prepared: &PreparedFound,
    lock_receipt: &ProjectedCustodyLockReceiptV1,
    lock_receipt_bytes: &[u8],
    product: ProductFacts,
    projected: ProjectedFacts,
    rent: &Rent,
) -> Result<PermitPlan, CoreSbfError> {
    let ticket_context = admitted.ticket.content_id().to_bytes();
    let permit_seeds = SeriesFoundingPermitSeedsV1::new(
        identity(request.release_set().to_bytes())?,
        identity(frame.found.market.key.to_bytes())?,
        identity(ticket_context)?,
    );
    let (expected_permit, bump) =
        Pubkey::find_program_address(&permit_seeds.as_slices(), program_id);
    if suffix.permit.key != &expected_permit
        || suffix.permit.owner != &system_program::ID
        || !suffix.permit.data_is_empty()
        || suffix.permit.lamports() < rent.minimum_balance(SERIES_FOUNDING_PERMIT_BYTES_V1)
        || suffix.founder.key.to_bytes()
            != request
                .founder()
                .ok_or(CoreSbfError::Instruction)?
                .to_bytes()
    {
        return Err(CoreSbfError::Creation);
    }
    let intent = build_intent(
        bump,
        frame,
        suffix,
        request,
        prepared,
        ticket_context,
        product,
        projected,
    )?;
    let intent_digest = digest_intent(&intent)?;
    let claims_request_digest = build_claims_request_digest(
        frame,
        suffix,
        request,
        prepared,
        lock_receipt,
        lock_receipt_bytes,
        product,
        projected,
        intent_digest,
        rent,
    )?;
    finish_permit(&intent, intent_digest, claims_request_digest)
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn build_intent(
    bump: u8,
    frame: &SeriesConsumeAccounts<'_, '_>,
    suffix: SeriesFoundSuffix<'_, '_>,
    request: SeriesCoreRequestV1,
    prepared: &PreparedFound,
    ticket_context: [u8; 32],
    product: ProductFacts,
    projected: ProjectedFacts,
) -> Result<Box<FoundingIntentV5>, CoreSbfError> {
    Ok(Box::new(
        FoundingIntentV5::new(
            bump,
            identity(request.release_set().to_bytes())?,
            identity(frame.found.market.key.to_bytes())?,
            identity(prepared.product_record_id)?,
            identity(prepared.resolution_policy_id)?,
            identity(suffix.founder.key.to_bytes())?,
            identity(ticket_context)?,
            identity(frame.root.key.to_bytes())?,
            identity(suffix.projected_replay.key.to_bytes())?,
            identity(suffix.funding_source.key.to_bytes())?,
            identity(suffix.hoard.key.to_bytes())?,
            identity(projected.realize_request_digest)?,
            identity(projected.realize_receipt_digest)?,
            identity(frame.trading_program.key.to_bytes())?,
            identity(suffix.claims_program.key.to_bytes())?,
            identity(frame.found.rent_credit.key.to_bytes())?,
            request
                .market_generation()
                .ok_or(CoreSbfError::Instruction)?,
            projected.quantity,
            product.basis_scale,
            projected.expiry_slot,
            projected.realize_revision,
            1,
        )
        .map_err(|_| CoreSbfError::Reference)?,
    ))
}

#[inline(never)]
fn digest_intent(intent: &FoundingIntentV5) -> Result<[u8; 32], CoreSbfError> {
    Ok(hash(&intent.encode().map_err(|_| CoreSbfError::Reference)?).to_bytes())
}

#[inline(never)]
fn finish_permit(
    intent: &FoundingIntentV5,
    intent_digest: [u8; 32],
    request_digest: [u8; 32],
) -> Result<PermitPlan, CoreSbfError> {
    Ok(PermitPlan {
        permit: Box::new(
            SeriesFoundingPermitV1::new(
                *intent,
                identity(intent_digest)?,
                identity(request_digest)?,
            )
            .map_err(|_| CoreSbfError::Reference)?,
        ),
    })
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn build_claims_request_digest(
    frame: &SeriesConsumeAccounts<'_, '_>,
    suffix: SeriesFoundSuffix<'_, '_>,
    request: SeriesCoreRequestV1,
    prepared: &PreparedFound,
    lock_receipt: &ProjectedCustodyLockReceiptV1,
    lock_receipt_bytes: &[u8],
    product: ProductFacts,
    projected: ProjectedFacts,
    intent_digest: [u8; 32],
    rent: &Rent,
) -> Result<[u8; 32], CoreSbfError> {
    let aggregate_seeds = ClaimsFoundingAggregateSeedsV5::new(frame.found.market.key.to_bytes())
        .map_err(|_| CoreSbfError::Reference)?;
    let expected_aggregate =
        Pubkey::find_program_address(&aggregate_seeds.as_slices(), suffix.claims_program.key).0;
    let position_seeds =
        ProtocolPositionSeedsV2::new(expected_aggregate.to_bytes(), suffix.founder.key.to_bytes())
            .map_err(|_| CoreSbfError::Reference)?;
    let expected_position =
        Pubkey::find_program_address(&position_seeds.as_slices(), suffix.claims_program.key).0;
    let admission_seeds = ProtocolPositionAdmissionSeedsV2::new(
        expected_aggregate.to_bytes(),
        suffix.founder.key.to_bytes(),
    )
    .map_err(|_| CoreSbfError::Reference)?;
    let expected_admission =
        Pubkey::find_program_address(&admission_seeds.as_slices(), suffix.claims_program.key).0;
    if suffix.aggregate.key != &expected_aggregate
        || suffix.position.key != &expected_position
        || suffix.admission.key != &expected_admission
    {
        return Err(CoreSbfError::Reference);
    }
    for vacant in [suffix.aggregate, suffix.position, suffix.admission] {
        if vacant.owner != &system_program::ID || !vacant.data_is_empty() || vacant.executable {
            return Err(CoreSbfError::Creation);
        }
    }
    let aggregate_width = liability_basis_vector_width_v2(
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
        product.claim_count,
    )
    .map_err(|_| CoreSbfError::Arithmetic)?;
    let position_width = liability_basis_vector_width_v2(
        LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        product.claim_count,
    )
    .map_err(|_| CoreSbfError::Arithmetic)?;
    let claims_request = ClaimsFoundingRequestV5::new(ClaimsFoundingRequestInputV5 {
        release_set: request.release_set().to_bytes(),
        market: frame.found.market.key.to_bytes(),
        product_record_digest: prepared.product_record_id,
        product_instance_id: prepared.product_id,
        linked_basis_record_digest: product.linked_basis_record,
        semantic_basis_id: product.semantic_basis,
        founder: suffix.founder.key.to_bytes(),
        founding_intent_digest: intent_digest,
        aggregate: suffix.aggregate.key.to_bytes(),
        position: suffix.position.key.to_bytes(),
        admission: suffix.admission.key.to_bytes(),
        hoard: suffix.hoard.key.to_bytes(),
        rent_credit: frame.found.rent_credit.key.to_bytes(),
        rent_program: frame.found.rent_program.key.to_bytes(),
        claims_program: suffix.claims_program.key.to_bytes(),
        trading_program: frame.trading_program.key.to_bytes(),
        funding_source: suffix.funding_source.key.to_bytes(),
        custody_replay: suffix.projected_replay.key.to_bytes(),
        custody_request_digest: lock_receipt.request_digest,
        custody_receipt_digest: hash(lock_receipt_bytes).to_bytes(),
        generation: request
            .market_generation()
            .ok_or(CoreSbfError::Instruction)?,
        claim_count: product.claim_count,
        quantity: projected.quantity,
        basis_scale: product.basis_scale,
        pre_source_amount: request.hoard_principal(),
        post_source_amount: 0,
        pre_hoard_amount: 0,
        post_hoard_amount: projected.hoard_amount,
        pre_custody_revision: 0,
        post_custody_revision: 1,
        aggregate_rent_principal: rent.minimum_balance(aggregate_width),
        position_rent_principal: rent.minimum_balance(position_width),
        admission_rent_principal: rent.minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2),
        observed_aggregate_lamports: suffix.aggregate.lamports(),
        observed_position_lamports: suffix.position.lamports(),
        observed_admission_lamports: suffix.admission.lamports(),
        pre_aggregate_revision: 0,
        post_aggregate_revision: 1,
        pre_position_revision: 0,
        post_position_revision: 1,
    })
    .map_err(|_| CoreSbfError::Reference)?;
    Ok(hash(&claims_request.to_bytes()).to_bytes())
}

#[inline(never)]
fn create_permit<'accounts, 'info>(
    program_id: &Pubkey,
    suffix: SeriesFoundSuffix<'accounts, 'info>,
    system: &'accounts AccountInfo<'info>,
    permit: &SeriesFoundingPermitV1,
    rent: &Rent,
) -> Result<(), CoreSbfError> {
    if suffix.permit.owner != &system_program::ID
        || !suffix.permit.data_is_empty()
        || suffix.permit.lamports() < rent.minimum_balance(SERIES_FOUNDING_PERMIT_BYTES_V1)
    {
        return Err(CoreSbfError::Creation);
    }
    let seeds = permit.seeds();
    let base = seeds.as_slices();
    let bump = [permit.intent().bump()];
    let signer = [base[0], base[1], base[2], base[3], bump.as_slice()];
    for instruction in [
        allocate(
            suffix.permit.key,
            u64::try_from(SERIES_FOUNDING_PERMIT_BYTES_V1).map_err(|_| CoreSbfError::Arithmetic)?,
        ),
        assign(suffix.permit.key, program_id),
    ] {
        invoke_signed(
            &instruction,
            &[suffix.permit.clone(), system.clone()],
            &[&signer],
        )
        .map_err(|_| CoreSbfError::Creation)?;
    }
    let encoded = permit.encode().map_err(|_| CoreSbfError::Commit)?;
    {
        let mut data = suffix
            .permit
            .try_borrow_mut_data()
            .map_err(|_| CoreSbfError::Commit)?;
        if data.len() != SERIES_FOUNDING_PERMIT_BYTES_V1 {
            return Err(CoreSbfError::Commit);
        }
        data.copy_from_slice(&encoded);
    }
    let data = suffix
        .permit
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Commit)?;
    if suffix.permit.owner != program_id || SeriesFoundingPermitV1::decode(&data) != Ok(*permit) {
        return Err(CoreSbfError::Commit);
    }
    Ok(())
}

fn ticket_content_id_from_account(account: &AccountInfo<'_>) -> Result<[u8; 32], CoreSbfError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    Ok(ticket_content_id(&data)
        .map_err(|_| CoreSbfError::Reference)?
        .to_bytes())
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, CoreSbfError> {
    accounts.get(index).ok_or(CoreSbfError::AccountFrame)
}
