//! Final recurring-Series Core opening after Custody realization and Claims V5.

use alloc::boxed::Box;

use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
};
use dclutch_claims_svm::founding_v5::ClaimsFoundingReceiptV5;
use dclutch_market_core_codec::{
    CoreState, MarketCoreStateSeedsV2, Readiness, Role, SERIES_OPEN_POST_RESOURCE_DIGEST_DOMAIN_V1,
    STATE_BYTES, SeriesCoreAckV1, SeriesCoreActionV1, SeriesCoreRequestV1, SeriesFoundingPermitV1,
};
use dclutch_product_runtime_v2_svm_reader::{
    FinalizedRecordFrameV2, ProductRuntimeFrameV2, authenticate_product_runtime_v2,
};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
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
    generic_founding_v1::{
        GenericFoundingOpenAccounts, apply_open, authenticate_claims_and_custody,
        authenticate_permit as authenticate_generic_permit,
        authenticate_rent_credit as authenticate_generic_rent_credit,
        close_permit as close_generic_permit, commit_market as commit_generic_market,
        decode_claims_receipt as decode_generic_claims_receipt,
    },
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

    fn generic(&self) -> GenericFoundingOpenAccounts<'_, 'info> {
        GenericFoundingOpenAccounts {
            market: self.market,
            permit: self.permit,
            rent_credit: self.rent_credit,
            rent_program: self.rent_program,
            trading_program: self.trading_program,
            claims_program: self.claims_program,
            custody_program: self.custody_program,
            capability_root: self.root,
            custody_replay: self.custody_replay,
            hoard: self.hoard,
            funding_source: self.funding_source,
            aggregate: self.aggregate,
            position: self.position,
            admission: self.admission,
        }
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
    let generic = frame.generic();
    let permit = authenticate_generic_permit(
        program_id,
        &generic,
        ticket_context,
        request
            .founder()
            .ok_or(CoreSbfError::Instruction)?
            .to_bytes(),
        &rent,
        clock.slot,
        *state,
    )?;
    authenticate_generic_rent_credit(
        &generic,
        request.beneficiary().to_bytes(),
        request.release_set().to_bytes(),
        request
            .market_generation()
            .ok_or(CoreSbfError::Instruction)?,
        &rent,
    )?;
    let claims_receipt = decode_generic_claims_receipt(claims_receipt_bytes)?;
    authenticate_claims_and_custody(&generic, &permit, &claims_receipt, *state)?;
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
    apply_open(state, receipt, claims_admission, custody_admission)
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
    commit_generic_market(frame.market, state, program_id)?;
    close_generic_permit(frame.permit, frame.rent_credit, program_id)?;
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
fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, CoreSbfError> {
    accounts.get(index).ok_or(CoreSbfError::AccountFrame)
}
