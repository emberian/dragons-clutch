//! Final recurring-Series Core opening after Custody realization and Claims V5.

use alloc::boxed::Box;

use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
};
use dclutch_claims_svm::founding_v5::{
    CLAIMS_FOUNDING_POST_RESOURCE_DIGEST_DOMAIN_V5, ClaimsFoundingReceiptV5,
};
use dclutch_custody_contract::{
    CUSTODY_REPLAY_BYTES_V1, CallerRoleV1, CustodyReplayV1, PROJECTED_HOARD_CONTEXT_DOMAIN_V1,
};
use dclutch_market_core_codec::{
    Action, Admission, ChildEffectObservation, CoreState, MarketCoreStateSeedsV2, Readiness,
    Request, Role, SERIES_FOUNDING_PERMIT_BYTES_V1, STATE_BYTES, SeriesCoreAckV1,
    SeriesCoreActionV1, SeriesCoreRequestV1, SeriesFoundingPermitV1, SeriesOpenObservation,
    SERIES_OPEN_POST_RESOURCE_DIGEST_DOMAIN_V1, open_series_market,
};
use dclutch_product_runtime_v2_svm_reader::{
    FinalizedRecordFrameV2, ProductRuntimeFrameV2, authenticate_product_runtime_v2,
};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_rent_contract::lifecycle_v2::{LIFECYCLE_RENT_CREDIT_BYTES_V2, LifecycleRentCreditV2};
use dclutch_series_v3_kernel::{
    AccountKeyV3, AuthenticatedProductProjectionV2, SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3,
    SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3, SERIES_TICKET_SCHEMA_RELEASE_ID_V3,
    admit_occurrence_bytes, admit_ticket, future_market_projection,
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
    program::set_return_data,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{system_program, sysvar};

use crate::{
    CoreSbfError,
    frame::require_distinct,
    records::authenticate_finalized_record,
    release::{authenticate_role, identity},
};

/// Exact final-Series-Open account count.
pub const SERIES_OPEN_ACCOUNT_COUNT_V1: usize = 37;
struct SeriesOpenAccounts<'accounts, 'info> {
    caller: &'accounts AccountInfo<'info>,
    market: &'accounts AccountInfo<'info>,
    permit: &'accounts AccountInfo<'info>,
    rent_credit: &'accounts AccountInfo<'info>,
    rent_program: &'accounts AccountInfo<'info>,
    activation_cache: &'accounts AccountInfo<'info>,
    registry_program: &'accounts AccountInfo<'info>,
    trading_program: &'accounts AccountInfo<'info>,
    trading_programdata: &'accounts AccountInfo<'info>,
    claims_program: &'accounts AccountInfo<'info>,
    claims_programdata: &'accounts AccountInfo<'info>,
    custody_program: &'accounts AccountInfo<'info>,
    custody_programdata: &'accounts AccountInfo<'info>,
    core_program: &'accounts AccountInfo<'info>,
    core_programdata: &'accounts AccountInfo<'info>,
    root: &'accounts AccountInfo<'info>,
    ticket_state: &'accounts AccountInfo<'info>,
    template_raw: &'accounts AccountInfo<'info>,
    template_staging: &'accounts AccountInfo<'info>,
    occurrence_raw: &'accounts AccountInfo<'info>,
    occurrence_staging: &'accounts AccountInfo<'info>,
    ticket_raw: &'accounts AccountInfo<'info>,
    ticket_staging: &'accounts AccountInfo<'info>,
    product_raw: &'accounts AccountInfo<'info>,
    product_staging: &'accounts AccountInfo<'info>,
    result_domain_raw: &'accounts AccountInfo<'info>,
    result_domain_staging: &'accounts AccountInfo<'info>,
    portfolio_raw: &'accounts AccountInfo<'info>,
    portfolio_staging: &'accounts AccountInfo<'info>,
    custody_replay: &'accounts AccountInfo<'info>,
    hoard: &'accounts AccountInfo<'info>,
    funding_source: &'accounts AccountInfo<'info>,
    aggregate: &'accounts AccountInfo<'info>,
    position: &'accounts AccountInfo<'info>,
    admission: &'accounts AccountInfo<'info>,
    clock: &'accounts AccountInfo<'info>,
    rent: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> SeriesOpenAccounts<'accounts, 'info> {
    #[inline(never)]
    fn parse(
        program_id: &Pubkey,
        accounts: &'accounts [AccountInfo<'info>],
    ) -> Result<Self, CoreSbfError> {
        if accounts.len() != SERIES_OPEN_ACCOUNT_COUNT_V1 {
            return Err(CoreSbfError::AccountFrame);
        }
        require_distinct(accounts)?;
        let value = Self {
            caller: account(accounts, 0)?,
            market: account(accounts, 1)?,
            permit: account(accounts, 2)?,
            rent_credit: account(accounts, 3)?,
            rent_program: account(accounts, 4)?,
            activation_cache: account(accounts, 5)?,
            registry_program: account(accounts, 6)?,
            trading_program: account(accounts, 7)?,
            trading_programdata: account(accounts, 8)?,
            claims_program: account(accounts, 9)?,
            claims_programdata: account(accounts, 10)?,
            custody_program: account(accounts, 11)?,
            custody_programdata: account(accounts, 12)?,
            core_program: account(accounts, 13)?,
            core_programdata: account(accounts, 14)?,
            root: account(accounts, 15)?,
            ticket_state: account(accounts, 16)?,
            template_raw: account(accounts, 17)?,
            template_staging: account(accounts, 18)?,
            occurrence_raw: account(accounts, 19)?,
            occurrence_staging: account(accounts, 20)?,
            ticket_raw: account(accounts, 21)?,
            ticket_staging: account(accounts, 22)?,
            product_raw: account(accounts, 23)?,
            product_staging: account(accounts, 24)?,
            result_domain_raw: account(accounts, 25)?,
            result_domain_staging: account(accounts, 26)?,
            portfolio_raw: account(accounts, 27)?,
            portfolio_staging: account(accounts, 28)?,
            custody_replay: account(accounts, 29)?,
            hoard: account(accounts, 30)?,
            funding_source: account(accounts, 31)?,
            aggregate: account(accounts, 32)?,
            position: account(accounts, 33)?,
            admission: account(accounts, 34)?,
            clock: account(accounts, 35)?,
            rent: account(accounts, 36)?,
        };
        if !value.caller.is_signer
            || value.caller.is_writable
            || value.caller.executable
            || value.market.is_signer
            || !value.market.is_writable
            || value.market.executable
            || value.permit.is_signer
            || !value.permit.is_writable
            || value.permit.executable
            || value.rent_credit.is_signer
            || !value.rent_credit.is_writable
            || value.rent_credit.executable
            || value.registry_program.is_signer
            || value.registry_program.is_writable
            || !value.registry_program.executable
            || value.rent_program.is_signer
            || value.rent_program.is_writable
            || !value.rent_program.executable
            || value.core_program.key != program_id
            || !value.core_program.executable
            || value.clock.key != &sysvar::clock::ID
            || value.clock.is_signer
            || value.clock.is_writable
            || value.clock.executable
            || value.rent.key != &sysvar::rent::ID
            || value.rent.is_signer
            || value.rent.is_writable
            || value.rent.executable
        {
            return Err(CoreSbfError::AccountFrame);
        }
        for program in [
            value.trading_program,
            value.claims_program,
            value.custody_program,
            value.core_program,
        ] {
            if program.is_signer || program.is_writable || !program.executable {
                return Err(CoreSbfError::AccountFrame);
            }
        }
        for readonly in [
            value.activation_cache,
            value.trading_programdata,
            value.claims_programdata,
            value.custody_programdata,
            value.core_programdata,
            value.root,
            value.ticket_state,
            value.template_raw,
            value.template_staging,
            value.occurrence_raw,
            value.occurrence_staging,
            value.ticket_raw,
            value.ticket_staging,
            value.product_raw,
            value.product_staging,
            value.result_domain_raw,
            value.result_domain_staging,
            value.portfolio_raw,
            value.portfolio_staging,
            value.custody_replay,
            value.hoard,
            value.funding_source,
            value.aggregate,
            value.position,
            value.admission,
        ] {
            if readonly.is_signer || readonly.is_writable || readonly.executable {
                return Err(CoreSbfError::AccountFrame);
            }
        }
        Ok(value)
    }
}

struct ReplayCandidates {
    root: [u8; CAPABILITY_ROOT_HEADER_BYTES_V1 + SERIES_STATE_BYTES_V3],
    ticket: [u8; SERIES_TICKET_STATE_BYTES_V3],
}

/// Authenticate exact Claims V5 completion and commit the Market Open last.
#[inline(never)]
pub(crate) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: SeriesCoreRequestV1,
    request_bytes: &[u8],
    proof_bytes: &[u8],
    claims_receipt_bytes: &[u8],
) -> Result<(), solana_program::program_error::ProgramError> {
    if request.action() != SeriesCoreActionV1::Consume {
        return Err(CoreSbfError::Instruction.into());
    }
    let frame = SeriesOpenAccounts::parse(program_id, accounts)?;
    let rent = Rent::from_account_info(frame.rent).map_err(|_| CoreSbfError::Creation)?;
    let clock = Clock::from_account_info(frame.clock).map_err(|_| CoreSbfError::Creation)?;
    let mut state = authenticate_market_and_roles(program_id, &frame, request)?;
    let (ticket_context, replay_candidates) =
        authenticate_series(&frame, request, proof_bytes, &rent, *state)?;
    authenticate_caller(&frame, request, request_bytes, ticket_context)?;
    let permit = authenticate_permit(
        program_id,
        &frame,
        request,
        ticket_context,
        &rent,
        clock.slot,
        *state,
    )?;
    authenticate_rent_credit(&frame, request, &rent)?;
    let claims_receipt = decode_claims_receipt(claims_receipt_bytes)?;
    authenticate_claims_receipt(&frame, request, &permit, &claims_receipt, *state)?;
    authenticate_roles_and_apply(&frame, request, &claims_receipt, &mut state)?;
    commit_and_ack(
        program_id,
        &frame,
        request,
        request_bytes,
        claims_receipt_bytes,
        *state,
        &replay_candidates,
        &permit,
        ticket_context,
    )
}

#[inline(never)]
fn authenticate_roles_and_apply(
    frame: &SeriesOpenAccounts<'_, '_>,
    request: SeriesCoreRequestV1,
    receipt: &ClaimsFoundingReceiptV5,
    state: &mut CoreState,
) -> Result<(), CoreSbfError> {
    let claims_admission = authenticate_role(
        frame.activation_cache,
        frame.registry_program,
        frame.claims_program,
        frame.claims_programdata,
        state.identity.registry_program,
        request.release_set().to_bytes(),
        Role::Claims,
    )?;
    let custody_admission = authenticate_role(
        frame.activation_cache,
        frame.registry_program,
        frame.custody_program,
        frame.custody_programdata,
        state.identity.registry_program,
        request.release_set().to_bytes(),
        Role::Custody,
    )?;
    apply_series_open(state, receipt, claims_admission, custody_admission)
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn commit_and_ack(
    program_id: &Pubkey,
    frame: &SeriesOpenAccounts<'_, '_>,
    request: SeriesCoreRequestV1,
    request_bytes: &[u8],
    claims_receipt_bytes: &[u8],
    state: CoreState,
    replay_candidates: &ReplayCandidates,
    permit: &SeriesFoundingPermitV1,
    ticket_context: [u8; 32],
) -> Result<(), solana_program::program_error::ProgramError> {
    commit_market(frame.market, state, program_id)?;
    close_permit(frame.permit, frame.rent_credit, program_id)?;
    let market_data = frame
        .market
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Commit)?;
    let post_resource_digest = hashv(&[
        SERIES_OPEN_POST_RESOURCE_DIGEST_DOMAIN_V1,
        &market_data,
        claims_receipt_bytes,
        &replay_candidates.root,
        &replay_candidates.ticket,
    ])
    .to_bytes();
    drop(market_data);
    if frame.permit.owner != &system_program::ID
        || frame.permit.lamports() != 0
        || !frame.permit.data_is_empty()
        || permit.intent().ticket_context().to_bytes() != ticket_context
    {
        return Err(CoreSbfError::Commit.into());
    }
    let acknowledgement = SeriesCoreAckV1::new(
        request,
        identity(program_id.to_bytes())?,
        identity(hash(request_bytes).to_bytes())?,
        identity(post_resource_digest)?,
    );
    let bytes = acknowledgement
        .encode()
        .map_err(|_| CoreSbfError::ChildAck)?;
    set_return_data(&bytes);
    Ok(())
}

#[inline(never)]
fn authenticate_market_and_roles(
    program_id: &Pubkey,
    frame: &SeriesOpenAccounts<'_, '_>,
    request: SeriesCoreRequestV1,
) -> Result<Box<CoreState>, CoreSbfError> {
    if frame.market.owner != program_id || frame.market.data_len() != STATE_BYTES {
        return Err(CoreSbfError::Market);
    }
    let data = frame
        .market
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Market)?;
    let state = CoreState::decode(&data).map_err(|_| CoreSbfError::Market)?;
    drop(data);
    let seeds = MarketCoreStateSeedsV2::new(state.identity);
    if Pubkey::find_program_address(&seeds.as_slices(), program_id).0 != *frame.market.key
        || state.identity.market_id.to_bytes() != frame.market.key.to_bytes()
        || state.identity.registry_program.to_bytes() != frame.registry_program.key.to_bytes()
        || state.identity.selected_release_set.to_bytes() != request.release_set().to_bytes()
        || state.identity.generation
            != request
                .market_generation()
                .ok_or(CoreSbfError::Instruction)?
        || state.identity.product_record.to_bytes()
            != request
                .product()
                .ok_or(CoreSbfError::Instruction)?
                .to_bytes()
        || state.rent_beneficiary.to_bytes() != frame.rent_credit.key.to_bytes()
        || !matches!(state.phase, dclutch_market_core_codec::Phase::Founding)
        || state.readiness != Readiness::Prepaid
    {
        return Err(CoreSbfError::Market);
    }
    for (role, role_program, role_programdata) in [
        (Role::Core, frame.core_program, frame.core_programdata),
        (
            Role::Trading,
            frame.trading_program,
            frame.trading_programdata,
        ),
    ] {
        authenticate_role(
            frame.activation_cache,
            frame.registry_program,
            role_program,
            role_programdata,
            state.identity.registry_program,
            request.release_set().to_bytes(),
            role,
        )?;
    }
    Ok(Box::new(state))
}

#[inline(never)]
fn authenticate_series(
    frame: &SeriesOpenAccounts<'_, '_>,
    request: SeriesCoreRequestV1,
    proof_bytes: &[u8],
    rent: &Rent,
    state: CoreState,
) -> Result<([u8; 32], Box<ReplayCandidates>), CoreSbfError> {
    for (raw, staging, schema) in [
        (
            frame.template_raw,
            frame.template_staging,
            SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
        ),
        (
            frame.occurrence_raw,
            frame.occurrence_staging,
            SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3,
        ),
        (
            frame.ticket_raw,
            frame.ticket_staging,
            SERIES_TICKET_SCHEMA_RELEASE_ID_V3,
        ),
    ] {
        let bytes = raw
            .try_borrow_data()
            .map_err(|_| CoreSbfError::FinalizedRecord)?;
        authenticate_finalized_record(
            frame.registry_program.key,
            raw,
            staging,
            rent,
            schema,
            hash(&bytes).to_bytes(),
            &bytes,
        )?;
    }
    let template_bytes = frame
        .template_raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    let occurrence_bytes = frame
        .occurrence_raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    let ticket_bytes = frame
        .ticket_raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    let occurrence = admit_occurrence_bytes(&template_bytes, &occurrence_bytes, proof_bytes)
        .map_err(|_| CoreSbfError::Reference)?;
    let ticket = admit_ticket(&ticket_bytes).map_err(|_| CoreSbfError::Reference)?;
    occurrence
        .require_ticket(ticket.ticket())
        .map_err(|_| CoreSbfError::Reference)?;
    let ticket_context = ticket_content_id(&ticket_bytes)
        .map_err(|_| CoreSbfError::Reference)?
        .to_bytes();
    if template_content_id(&template_bytes)
        .map_err(|_| CoreSbfError::Reference)?
        .to_bytes()
        != request.template().to_bytes()
        || request
            .ticket()
            .ok_or(CoreSbfError::Instruction)?
            .to_bytes()
            != frame.ticket_state.key.to_bytes()
    {
        return Err(CoreSbfError::Reference);
    }
    let product = authenticate_product_runtime_v2(
        frame.registry_program.key,
        rent,
        dclutch_product_runtime_v2::ContentId::new(state.identity.product_record.to_bytes())
            .map_err(|_| CoreSbfError::Reference)?,
        ProductRuntimeFrameV2 {
            product: FinalizedRecordFrameV2 {
                raw: frame.product_raw,
                staging: frame.product_staging,
            },
            result_domain: FinalizedRecordFrameV2 {
                raw: frame.result_domain_raw,
                staging: frame.result_domain_staging,
            },
            portfolio: FinalizedRecordFrameV2 {
                raw: frame.portfolio_raw,
                staging: frame.portfolio_staging,
            },
        },
    )
    .map_err(|_| CoreSbfError::Reference)?;
    if product.product_id.to_bytes() != state.identity.product_id.to_bytes() {
        return Err(CoreSbfError::Reference);
    }
    let projected_product = AuthenticatedProductProjectionV2::new(
        dclutch_core_contract::ContentId::new(product.product_record.content_digest.to_bytes())
            .map_err(|_| CoreSbfError::Reference)?,
        dclutch_core_contract::ContentId::new(product.product_id.to_bytes())
            .map_err(|_| CoreSbfError::Reference)?,
        dclutch_core_contract::ContentId::new(
            product.result_domain_record.content_digest.to_bytes(),
        )
        .map_err(|_| CoreSbfError::Reference)?,
    );
    let future = future_market_projection(
        occurrence,
        projected_product,
        AccountKeyV3::new(frame.registry_program.key.to_bytes())
            .map_err(|_| CoreSbfError::Reference)?,
    )
    .map_err(|_| CoreSbfError::Reference)?;
    future
        .require_address(
            AccountKeyV3::new(frame.market.key.to_bytes()).map_err(|_| CoreSbfError::Reference)?,
        )
        .map_err(|_| CoreSbfError::Reference)?;
    if occurrence.occurrence().market().to_bytes() != frame.market.key.to_bytes()
        || occurrence.template().release_set().to_bytes() != request.release_set().to_bytes()
        || occurrence.template().realm().to_bytes() != state.identity.realm_id.to_bytes()
        || occurrence.occurrence().resolution_policy().to_bytes()
            != state.identity.resolution_policy.to_bytes()
        || occurrence.occurrence().capability_manifest().to_bytes()
            != state.identity.capability_manifest.to_bytes()
        || ticket.ticket().founder().to_bytes()
            != request
                .founder()
                .ok_or(CoreSbfError::Instruction)?
                .to_bytes()
        || ticket.ticket().refund_owner().to_bytes() != request.beneficiary().to_bytes()
        || ticket.ticket().funds().hoard_principal() != request.hoard_principal()
        || occurrence.occurrence().occurrence() != request.occurrence_index()
    {
        return Err(CoreSbfError::Reference);
    }
    drop(template_bytes);
    drop(occurrence_bytes);
    drop(ticket_bytes);
    authenticate_replay_candidates(frame, request, occurrence, ticket, ticket_context)
        .map(|candidate| (ticket_context, Box::new(candidate)))
}

#[inline(never)]
fn authenticate_replay_candidates(
    frame: &SeriesOpenAccounts<'_, '_>,
    request: SeriesCoreRequestV1,
    occurrence: dclutch_series_v3_kernel::AdmittedOccurrenceV3,
    ticket: dclutch_series_v3_kernel::AdmittedTicketV3,
    ticket_context: [u8; 32],
) -> Result<ReplayCandidates, CoreSbfError> {
    if frame.root.owner != frame.trading_program.key
        || frame.root.data_len() != CAPABILITY_ROOT_HEADER_BYTES_V1 + SERIES_STATE_BYTES_V3
        || frame.ticket_state.owner != frame.trading_program.key
        || frame.ticket_state.data_len() != SERIES_TICKET_STATE_BYTES_V3
    {
        return Err(CoreSbfError::Reference);
    }
    let root_data = frame
        .root
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Reference)?;
    let header = CapabilityRootHeaderV1::decode(
        root_data
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or(CoreSbfError::Reference)?,
    )
    .map_err(|_| CoreSbfError::Reference)?;
    if Pubkey::find_program_address(&header.seeds().as_slices(), frame.trading_program.key).0
        != *frame.root.key
        || header.release_set().to_bytes() != request.release_set().to_bytes()
        || header.selection().config().to_bytes() != request.template().to_bytes()
    {
        return Err(CoreSbfError::Reference);
    }
    let current_root = SeriesStateV3::decode(
        root_data
            .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .ok_or(CoreSbfError::Reference)?,
        occurrence.template().occurrence_count(),
    )
    .map_err(|_| CoreSbfError::Reference)?;
    let next_root = current_root
        .settle_current(
            request.expected_series_revision(),
            occurrence.template().occurrence_count(),
        )
        .map_err(|_| CoreSbfError::Reference)?;
    let mut root_candidate = [0_u8; CAPABILITY_ROOT_HEADER_BYTES_V1 + SERIES_STATE_BYTES_V3];
    root_candidate.copy_from_slice(&root_data);
    root_candidate[CAPABILITY_ROOT_HEADER_BYTES_V1..].copy_from_slice(
        &next_root
            .encode(occurrence.template().occurrence_count())
            .map_err(|_| CoreSbfError::Reference)?,
    );
    drop(root_data);

    let ticket_data = frame
        .ticket_state
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Reference)?;
    let current_ticket =
        TicketStateV3::decode(&ticket_data).map_err(|_| CoreSbfError::Reference)?;
    let ticket_seeds = TicketStateSeedsV3::new(frame.root.key.to_bytes(), ticket.content_id());
    if Pubkey::find_program_address(&ticket_seeds.as_slices(), frame.trading_program.key).0
        != *frame.ticket_state.key
        || current_ticket.ticket_record_id().to_bytes() != ticket_context
    {
        return Err(CoreSbfError::Reference);
    }
    let next_ticket = current_ticket
        .settle(request.expected_ticket_revision(), TicketPhaseV3::Consumed)
        .map_err(|_| CoreSbfError::Reference)?;
    Ok(ReplayCandidates {
        root: root_candidate,
        ticket: next_ticket.encode(),
    })
}

fn authenticate_caller(
    frame: &SeriesOpenAccounts<'_, '_>,
    request: SeriesCoreRequestV1,
    request_bytes: &[u8],
    ticket_context: [u8; 32],
) -> Result<(), CoreSbfError> {
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        request.release_set().to_bytes(),
        request
            .market()
            .ok_or(CoreSbfError::Instruction)?
            .to_bytes(),
        ExecutionRoleV1::Trading,
        ticket_context,
        hash(request_bytes).to_bytes(),
    )
    .map_err(|_| CoreSbfError::CallerAuthority)?;
    if Pubkey::find_program_address(&seeds.as_slices(), frame.trading_program.key).0
        != *frame.caller.key
    {
        return Err(CoreSbfError::CallerAuthority);
    }
    Ok(())
}

#[inline(never)]
fn authenticate_permit(
    program_id: &Pubkey,
    frame: &SeriesOpenAccounts<'_, '_>,
    request: SeriesCoreRequestV1,
    ticket_context: [u8; 32],
    rent: &Rent,
    current_slot: u64,
    state: CoreState,
) -> Result<Box<SeriesFoundingPermitV1>, CoreSbfError> {
    if frame.permit.owner != program_id
        || frame.permit.data_len() != SERIES_FOUNDING_PERMIT_BYTES_V1
        || !rent.is_exempt(frame.permit.lamports(), SERIES_FOUNDING_PERMIT_BYTES_V1)
    {
        return Err(CoreSbfError::Reference);
    }
    let permit_data = frame
        .permit
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Reference)?;
    let permit =
        SeriesFoundingPermitV1::decode(&permit_data).map_err(|_| CoreSbfError::Reference)?;
    drop(permit_data);
    let intent = permit.intent();
    let permit_seeds = permit.seeds();
    let (expected_permit, bump) =
        Pubkey::find_program_address(&permit_seeds.as_slices(), program_id);
    if expected_permit != *frame.permit.key
        || bump != intent.bump()
        || intent.market().to_bytes() != frame.market.key.to_bytes()
        || intent.release_set().to_bytes() != request.release_set().to_bytes()
        || intent.ticket_context().to_bytes() != ticket_context
        || intent.product_record().to_bytes() != state.identity.product_record.to_bytes()
        || intent.source().to_bytes() != state.identity.resolution_policy.to_bytes()
        || intent.founder().to_bytes()
            != request
                .founder()
                .ok_or(CoreSbfError::Instruction)?
                .to_bytes()
        || intent.parent_root().to_bytes() != frame.root.key.to_bytes()
        || intent.projected_replay().to_bytes() != frame.custody_replay.key.to_bytes()
        || intent.funding_source().to_bytes() != frame.funding_source.key.to_bytes()
        || intent.hoard().to_bytes() != frame.hoard.key.to_bytes()
        || intent.trading_program().to_bytes() != frame.trading_program.key.to_bytes()
        || intent.claims_program().to_bytes() != frame.claims_program.key.to_bytes()
        || intent.rent_credit().to_bytes() != frame.rent_credit.key.to_bytes()
        || intent.generation() != state.identity.generation
        || current_slot > intent.expiry_slot()
    {
        return Err(CoreSbfError::Reference);
    }
    Ok(Box::new(permit))
}

#[inline(never)]
fn decode_claims_receipt(
    claims_receipt_bytes: &[u8],
) -> Result<Box<ClaimsFoundingReceiptV5>, CoreSbfError> {
    let receipt = ClaimsFoundingReceiptV5::decode(claims_receipt_bytes)
        .map_err(|_| CoreSbfError::ChildAck)?;
    Ok(Box::new(receipt))
}

#[inline(never)]
fn authenticate_claims_receipt(
    frame: &SeriesOpenAccounts<'_, '_>,
    request: SeriesCoreRequestV1,
    permit: &SeriesFoundingPermitV1,
    receipt: &ClaimsFoundingReceiptV5,
    state: CoreState,
) -> Result<(), CoreSbfError> {
    let intent = permit.intent();
    let claims_request = receipt.request();
    receipt
        .verify_for(&claims_request, permit.claims_request_digest().to_bytes())
        .map_err(|_| CoreSbfError::ChildAck)?;
    let intent_bytes = intent.encode().map_err(|_| CoreSbfError::ChildAck)?;
    let intent_digest = hash(&intent_bytes).to_bytes();
    permit
        .verify_for_intent_and_request(
            intent,
            identity(intent_digest)?,
            identity(receipt.request_digest())?,
        )
        .map_err(|_| CoreSbfError::ChildAck)?;
    if claims_request.release_set() != request.release_set().to_bytes()
        || claims_request.market() != frame.market.key.to_bytes()
        || claims_request.product_record_digest() != state.identity.product_record.to_bytes()
        || claims_request.product_instance_id() != state.identity.product_id.to_bytes()
        || claims_request.founder() != intent.founder().to_bytes()
        || claims_request.founding_intent_digest() != intent_digest
        || claims_request.aggregate() != frame.aggregate.key.to_bytes()
        || claims_request.position() != frame.position.key.to_bytes()
        || claims_request.admission() != frame.admission.key.to_bytes()
        || claims_request.funding_source() != frame.funding_source.key.to_bytes()
        || claims_request.hoard() != frame.hoard.key.to_bytes()
        || claims_request.custody_replay() != frame.custody_replay.key.to_bytes()
        || claims_request.rent_credit() != frame.rent_credit.key.to_bytes()
        || claims_request.rent_program() != frame.rent_program.key.to_bytes()
        || claims_request.claims_program() != frame.claims_program.key.to_bytes()
        || claims_request.trading_program() != frame.trading_program.key.to_bytes()
        || claims_request.generation() != state.identity.generation
        || claims_request.quantity() != intent.quantity()
        || claims_request.basis_scale() != intent.basis_scale()
        || claims_request.post_custody_revision() != intent.normal_replay_revision()
        || claims_request.post_source_amount() != 0
        || claims_request.pre_source_amount() != claims_request.collateral_transferred()
        || claims_request.post_hoard_amount() != claims_request.collateral_transferred()
    {
        return Err(CoreSbfError::ChildAck);
    }
    authenticate_claims_poststate(frame, receipt)?;
    let ticket_context = permit.intent().ticket_context().to_bytes();
    authenticate_custody_poststate(frame, claims_request, intent, ticket_context)?;
    Ok(())
}

#[inline(never)]
fn apply_series_open(
    state: &mut CoreState,
    receipt: &ClaimsFoundingReceiptV5,
    claims_admission: Admission,
    custody_admission: Admission,
) -> Result<(), CoreSbfError> {
    let request = receipt.request();
    let observation = SeriesOpenObservation {
        claims_admission,
        custody_admission,
        quantity: request.quantity(),
        basis_scale: request.basis_scale(),
        source_debit: request.pre_source_amount(),
        hoard_credit: request.post_hoard_amount(),
        hoard_funding_authenticated: true,
        found_state_bound_by_custody: true,
        claims_custody_join_authenticated: true,
        ticket_prepared_authenticated: true,
        ticket_consumed_candidate_authenticated: true,
        claims_effect: ChildEffectObservation {
            exact_request_authenticated: true,
            exact_receipt_authenticated: true,
            post_resource_authenticated: true,
        },
        custody_effect: ChildEffectObservation {
            exact_request_authenticated: true,
            exact_receipt_authenticated: true,
            post_resource_authenticated: true,
        },
    };
    open_series_market(
        Request::administrative(
            Action::OpenMarket,
            state.identity.generation,
            state.identity.market_id,
        ),
        state,
        observation,
    )
    .map_err(|_| CoreSbfError::Transition)
}

fn authenticate_rent_credit(
    frame: &SeriesOpenAccounts<'_, '_>,
    request: SeriesCoreRequestV1,
    rent: &Rent,
) -> Result<(), CoreSbfError> {
    if frame.rent_credit.owner != frame.rent_program.key
        || frame.rent_credit.data_len() != LIFECYCLE_RENT_CREDIT_BYTES_V2
        || !rent.is_exempt(frame.rent_credit.lamports(), LIFECYCLE_RENT_CREDIT_BYTES_V2)
    {
        return Err(CoreSbfError::RentCredit);
    }
    let data = frame
        .rent_credit
        .try_borrow_data()
        .map_err(|_| CoreSbfError::RentCredit)?;
    let credit = LifecycleRentCreditV2::decode(&data).map_err(|_| CoreSbfError::RentCredit)?;
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
    {
        return Err(CoreSbfError::RentCredit);
    }
    let seeds = credit.pda_seeds();
    let market = seeds.market().to_bytes();
    let generation = seeds.generation();
    let bump = [seeds.bump()];
    let expected = Pubkey::create_program_address(
        &[seeds.domain(), &market, &generation, bump.as_slice()],
        frame.rent_program.key,
    )
    .map_err(|_| CoreSbfError::RentCredit)?;
    if expected != *frame.rent_credit.key {
        return Err(CoreSbfError::RentCredit);
    }
    Ok(())
}

fn authenticate_claims_poststate(
    frame: &SeriesOpenAccounts<'_, '_>,
    receipt: &ClaimsFoundingReceiptV5,
) -> Result<(), CoreSbfError> {
    let request = receipt.request();
    for (account, expected, digest) in [
        (
            frame.aggregate,
            request.aggregate(),
            receipt.aggregate_digest(),
        ),
        (
            frame.position,
            request.position(),
            receipt.position_digest(),
        ),
        (
            frame.admission,
            request.admission(),
            receipt.admission_digest(),
        ),
    ] {
        if account.owner != frame.claims_program.key || account.key.to_bytes() != expected {
            return Err(CoreSbfError::ChildAck);
        }
        let data = account
            .try_borrow_data()
            .map_err(|_| CoreSbfError::ChildAck)?;
        if hash(&data).to_bytes() != digest {
            return Err(CoreSbfError::ChildAck);
        }
    }
    let aggregate = frame
        .aggregate
        .try_borrow_data()
        .map_err(|_| CoreSbfError::ChildAck)?;
    let position = frame
        .position
        .try_borrow_data()
        .map_err(|_| CoreSbfError::ChildAck)?;
    let admission = frame
        .admission
        .try_borrow_data()
        .map_err(|_| CoreSbfError::ChildAck)?;
    if hashv(&[
        CLAIMS_FOUNDING_POST_RESOURCE_DIGEST_DOMAIN_V5,
        &aggregate,
        &position,
        &admission,
    ])
    .to_bytes()
        != receipt.post_resource_digest()
    {
        return Err(CoreSbfError::ChildAck);
    }
    Ok(())
}

fn authenticate_custody_poststate(
    frame: &SeriesOpenAccounts<'_, '_>,
    request: dclutch_claims_svm::founding_v5::ClaimsFoundingRequestV5,
    intent: dclutch_market_core_codec::FoundingIntentV5,
    ticket_context: [u8; 32],
) -> Result<(), CoreSbfError> {
    if frame.funding_source.owner != &system_program::ID
        || frame.funding_source.lamports() != 0
        || !frame.funding_source.data_is_empty()
        || frame.custody_replay.owner != frame.custody_program.key
        || frame.custody_replay.data_len() != CUSTODY_REPLAY_BYTES_V1
        || TokenProgram::parse(frame.hoard.owner.to_bytes())
            .map_err(|_| CoreSbfError::ChildAck)?
            .program_id()
            != frame.hoard.owner.to_bytes()
    {
        return Err(CoreSbfError::ChildAck);
    }
    let hoard_data = frame
        .hoard
        .try_borrow_data()
        .map_err(|_| CoreSbfError::ChildAck)?;
    let hoard = TokenAccount::parse(&hoard_data).map_err(|_| CoreSbfError::ChildAck)?;
    if hoard.amount != request.post_hoard_amount()
        || hoard.state != AccountState::Initialized
        || !hoard.delegate.is_none()
        || hoard.delegated_amount != 0
        || !hoard.native_reserve.is_none()
        || !hoard.close_authority.is_none()
    {
        return Err(CoreSbfError::ChildAck);
    }
    let replay_data = frame
        .custody_replay
        .try_borrow_data()
        .map_err(|_| CoreSbfError::ChildAck)?;
    let replay = CustodyReplayV1::decode(&replay_data).map_err(|_| CoreSbfError::ChildAck)?;
    let projected_context =
        hashv(&[PROJECTED_HOARD_CONTEXT_DOMAIN_V1, ticket_context.as_slice()]).to_bytes();
    let replay_expected = Pubkey::find_program_address(
        &[
            dclutch_custody_contract::CUSTODY_REPLAY_PDA_DOMAIN_V1,
            &request.market(),
            &request.release_set(),
            &projected_context,
        ],
        frame.custody_program.key,
    )
    .0;
    if replay_expected != *frame.custody_replay.key
        || replay.caller_role != CallerRoleV1::Trading
        || replay.release_set != request.release_set()
        || replay.market != request.market()
        || replay.context != projected_context
        || replay.caller_program != request.trading_program()
        || replay.rent_refund != request.rent_credit()
        || replay.open_vault_count != 1
        || replay.next_revision != intent.normal_replay_revision()
        || replay.last_request_digest != intent.projected_request_digest().to_bytes()
        || replay.last_poststate_commitment != intent.projected_receipt_digest().to_bytes()
    {
        return Err(CoreSbfError::ChildAck);
    }
    Ok(())
}

fn commit_market(
    market: &AccountInfo<'_>,
    state: CoreState,
    program_id: &Pubkey,
) -> Result<(), CoreSbfError> {
    let encoded = state.encode().map_err(|_| CoreSbfError::Commit)?;
    market
        .try_borrow_mut_data()
        .map_err(|_| CoreSbfError::Commit)?
        .copy_from_slice(&encoded);
    let data = market.try_borrow_data().map_err(|_| CoreSbfError::Commit)?;
    if market.owner != program_id || CoreState::decode(&data) != Ok(state) {
        return Err(CoreSbfError::Commit);
    }
    Ok(())
}

fn close_permit(
    permit: &AccountInfo<'_>,
    rent_credit: &AccountInfo<'_>,
    program_id: &Pubkey,
) -> Result<(), CoreSbfError> {
    if permit.owner != program_id || permit.key == rent_credit.key {
        return Err(CoreSbfError::Commit);
    }
    let destination = rent_credit
        .lamports()
        .checked_add(permit.lamports())
        .ok_or(CoreSbfError::Arithmetic)?;
    permit
        .try_borrow_mut_data()
        .map_err(|_| CoreSbfError::Commit)?
        .fill(0);
    **permit
        .try_borrow_mut_lamports()
        .map_err(|_| CoreSbfError::Commit)? = 0;
    **rent_credit
        .try_borrow_mut_lamports()
        .map_err(|_| CoreSbfError::Commit)? = destination;
    permit.resize(0).map_err(|_| CoreSbfError::Commit)?;
    permit.assign(&system_program::ID);
    Ok(())
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, CoreSbfError> {
    accounts.get(index).ok_or(CoreSbfError::AccountFrame)
}
