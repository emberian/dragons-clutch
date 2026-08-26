//! Release-selected Trading-to-Core recurring-Series Consume admission.
//!
//! The accepted Claims founding effect is intentionally absent from this
//! tranche. This adapter authenticates every authority and prestate through
//! the transactional Found write, then refuses at that exact child boundary.
//! Solana rollback therefore restores the vacant Market byte-for-byte.

use core::cmp::min;

use dclutch_capability_contract::{
    CapabilityFundingDerivationV1, CapabilityManifestV1, ContentId as CapabilityContentId,
    FUNDING_STATE_BYTES, FundingCustodyObservationV1, FundingStateV1, FundingStatus,
};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
};
use dclutch_core_contract::ContentId;
use dclutch_market_core_codec::{Action, Request, Role, SeriesCoreActionV1, SeriesCoreRequestV1};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_rent_contract::RentCreditV1;
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
use solana_program::{
    account_info::AccountInfo, clock::Clock, hash::hash, pubkey::Pubkey, rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{system_program, sysvar};

use crate::{
    CoreSbfError,
    found::{self, PreparedFound},
    frame::{FOUND_ACCOUNT_COUNT_V2, FoundAccounts, require_distinct},
    records::authenticate_finalized_record,
    release::{authenticate_role, identity},
};

/// Fixed account count before the ordered FundingState prefix and Claims suffix.
pub const SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V1: usize = FOUND_ACCOUNT_COUNT_V2 + 11;
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

/// Authenticate one Series Consume and stop at the unavailable Claims effect.
#[inline(never)]
pub(crate) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: SeriesCoreRequestV1,
    request_bytes: &[u8],
    proof_bytes: &[u8],
) -> Result<(), solana_program::program_error::ProgramError> {
    if request.action() != SeriesCoreActionV1::Consume {
        return Err(CoreSbfError::Instruction.into());
    }
    let frame = SeriesConsumeAccounts::parse(program_id, accounts)?;
    let rent = Rent::from_account_info(frame.found.rent).map_err(|_| CoreSbfError::Creation)?;

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
    // This single preparation establishes the infrastructure root, immutable
    // Core release, Registry records, Runtime Product, and vacant Market plan.
    // No Market bytes are written until every Series precondition also joins.
    let prepared = found::prepare(program_id, &frame.found, found_request, &rent)?;
    authenticate_trading_caller(&frame, request, request_bytes)?;

    let admitted = authenticate_series(&frame, request, proof_bytes, &rent, program_id, &prepared)?;
    authenticate_root_and_replay(&frame, request, admitted, program_id)?;
    let (_funding, _claims_suffix) =
        split_funding_prefix(&frame, admitted, request, &rent, &prepared)?;
    authenticate_found_coordinates(&frame, request, admitted, &rent, &prepared)?;

    // Found is an intentional transactional prewrite: the accepted Claims
    // adapter authenticates the live Core Market. Returning an error at the
    // still-missing child seam rolls the allocation, bytes, and lamports back.
    found::apply_prepared(program_id, &frame.found, prepared)?;
    Err(CoreSbfError::ChildCpi.into())
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
    authenticate_role(
        frame.found.activation_cache,
        frame.found.registry_program,
        frame.trading_program,
        frame.trading_programdata,
        identity(frame.found.registry_program.key.to_bytes())?,
        request.release_set().to_bytes(),
        Role::Trading,
    )?;
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
) -> Result<AdmittedSeries, CoreSbfError> {
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
    Ok(AdmittedSeries {
        occurrence,
        ticket,
        product,
    })
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
    admitted: AdmittedSeries,
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
    admitted: AdmittedSeries,
    request: SeriesCoreRequestV1,
    rent: &Rent,
    prepared: &PreparedFound,
) -> Result<(&'a [AccountInfo<'info>], &'a [AccountInfo<'info>]), CoreSbfError> {
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
        if funding_list_id(keys.get(..count).ok_or(CoreSbfError::Arithmetic)?)
            .map_err(|_| CoreSbfError::Funding)?
            == admitted.ticket.ticket().funding_list()
        {
            matched = Some(count);
            break;
        }
    }
    let count = matched.ok_or(CoreSbfError::Funding)?;
    let funding = frame.tail.get(..count).ok_or(CoreSbfError::AccountFrame)?;
    let claims = frame.tail.get(count..).ok_or(CoreSbfError::AccountFrame)?;
    authenticate_funding(frame, admitted, request, funding, rent, prepared)?;
    Ok((funding, claims))
}

fn authenticate_funding(
    frame: &SeriesConsumeAccounts<'_, '_>,
    admitted: AdmittedSeries,
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
    admitted: AdmittedSeries,
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
    let credit = RentCreditV1::decode(&credit_data).map_err(|_| CoreSbfError::RentCredit)?;
    if credit.refund_authority().to_bytes() != request.beneficiary().to_bytes()
        || !rent.is_exempt(
            frame.found.rent_credit.lamports(),
            frame.found.rent_credit.data_len(),
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
