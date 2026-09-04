//! Shared physical finalization for every Claims FoundingV5 Market.
//!
//! Family wrappers authenticate how a founding context was selected. This
//! module alone authenticates the resulting one-shot Core permit, Claims
//! resources, canonical Custody replay/Hoard, and commit-last Market opening.

use alloc::boxed::Box;

use dclutch_capability_contract::{
    CapabilityFundingLedgerDerivationV2, CapabilityManifestV1, ContentId as CapabilityContentId,
    FundingLedgerStatusV2, FundingLedgerV2, funding_ledger_bytes_v2,
    validate_funding_ledger_masks_v2,
};
use dclutch_capability_program_contract::{CapabilityRootHeaderV1, SelectedRecordBumpsV1};
use dclutch_claims_svm::founding_v5::{
    CLAIMS_FOUNDING_POST_RESOURCE_DIGEST_DOMAIN_V5, ClaimsFoundingAggregateSeedsV5,
    ClaimsFoundingReceiptV5, ClaimsFoundingRequestInputV5, ClaimsFoundingRequestV5,
};
use dclutch_claims_svm::{
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        liability_basis_vector_width_v2,
    },
    protocol_position_v2::{
        PROTOCOL_POSITION_ADMISSION_BYTES_V2, ProtocolPositionAdmissionSeedsV2,
        ProtocolPositionSeedsV2,
    },
};
use dclutch_custody_contract::{
    CUSTODY_REPLAY_BYTES_V1, CallerRoleV1, CustodyReplayV1,
    PROJECTED_CUSTODY_LOCKED_ADMISSIBLE_STATES_V1, PROJECTED_CUSTODY_STATE_BYTES_V2,
    PROJECTED_HOARD_CONTEXT_DOMAIN_V1, ProjectedCustodyLockReceiptV1, ProjectedCustodyOperationV1,
    ProjectedCustodyStateSeedsV2, ProjectedCustodyStateV2,
};
use dclutch_market_core_codec::{
    Action, Admission, ChildEffectObservation, CoreState, FoundingIntentV5,
    GENERIC_FOUNDING_FOUND_POST_RESOURCE_DOMAIN_V1, GENERIC_FOUNDING_MAX_FUNDING_STATES_V1,
    GENERIC_FOUNDING_OPEN_POST_RESOURCE_DOMAIN_V1, GenericFoundingAckV1, GenericFoundingRequestV1,
    GenericFoundingStageV1, Identity, MarketAdmissionV1, MarketCoreStateSeedsV2, Phase, Readiness,
    Request, Role, SERIES_FOUNDING_PERMIT_BYTES_V1, STATE_BYTES, SeriesFoundingPermitSeedsV1,
    SeriesFoundingPermitV1, SeriesOpenObservation, generic_founding_funding_list_id_v1,
    open_series_market,
};
use dclutch_product_runtime_v2_svm_reader::{
    FinalizedRecordFrameV2, authenticate_founding_product_basis_v3,
};
use dclutch_release_set_contract::{
    CallerAuthoritySeedsV1, CapabilityExecutionSelectionV1, ExecutionRoleV1,
};
use dclutch_rent_contract::lifecycle_v2::{LIFECYCLE_RENT_CREDIT_BYTES_V2, LifecycleRentCreditV2};
use dclutch_source_contract::MarketPrincipalCapSetsV1;
use dclutch_token_svm::{AccountState, TokenAccount, TokenProgram};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    hash::{hash, hashv},
    program::{invoke_signed, set_return_data},
    pubkey::Pubkey,
    rent::Rent,
    sysvar::Sysvar,
};
use solana_sdk_ids::system_program;
use solana_system_interface::instruction::{allocate, assign};

use crate::{
    CoreSbfError,
    found::{self, PreparedFound, ProjectedFoundAuthorityV2},
    frame::{PROJECTED_FOUND_ACCOUNT_COUNT_V2, ProjectedFoundAccountsV2, require_distinct},
    release::{RoleBatchAdmissions, RoleDeploymentAccounts, authenticate_roles},
};

pub use dclutch_market_core_codec::{
    GENERIC_FOUNDING_FOUND_FIXED_ACCOUNT_COUNT_V1,
    GENERIC_FOUNDING_FOUND_PRICE_GATE_SUFFIX_ACCOUNT_COUNT_V2,
    GENERIC_FOUNDING_FOUND_SUFFIX_ACCOUNT_COUNT_V1, GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1,
};

const _: () =
    assert!(GENERIC_FOUNDING_FOUND_FIXED_ACCOUNT_COUNT_V1 == PROJECTED_FOUND_ACCOUNT_COUNT_V2 + 2);

/// Market prestates in which the generic founding's Open stage is admissible.
///
/// The Found stage of this same route CREATES the Market, so it has no
/// prestate to declare; this is the second stage, which finds the Market its
/// own Found stage left at `Founding + Prepaid` and opens it atomically.
pub const GENERIC_FOUNDING_OPEN_ADMISSIBLE_PRESTATES_V1: MarketAdmissionV1 =
    MarketAdmissionV1::prestates(&[(Phase::Founding, Readiness::Prepaid)]);

struct GenericFoundAccounts<'accounts, 'info> {
    found: ProjectedFoundAccountsV2<'accounts, 'info>,
    trading_program: &'accounts AccountInfo<'info>,
    trading_programdata: &'accounts AccountInfo<'info>,
    funding: &'accounts [AccountInfo<'info>],
    suffix: GenericFoundSuffix<'accounts, 'info>,
}

#[derive(Clone, Copy)]
struct GenericFoundSuffix<'accounts, 'info> {
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
    price_gate: Option<FinalizedRecordFrameV2<'accounts, 'info>>,
}

impl<'accounts, 'info> GenericFoundAccounts<'accounts, 'info> {
    fn parse(
        program_id: &Pubkey,
        accounts: &'accounts [AccountInfo<'info>],
        funding_count: usize,
    ) -> Result<Self, CoreSbfError> {
        let bare = GENERIC_FOUNDING_FOUND_FIXED_ACCOUNT_COUNT_V1
            .checked_add(funding_count)
            .and_then(|value| value.checked_add(GENERIC_FOUNDING_FOUND_SUFFIX_ACCOUNT_COUNT_V1))
            .ok_or(CoreSbfError::Arithmetic)?;
        let gated = GENERIC_FOUNDING_FOUND_FIXED_ACCOUNT_COUNT_V1
            .checked_add(funding_count)
            .and_then(|value| {
                value.checked_add(GENERIC_FOUNDING_FOUND_PRICE_GATE_SUFFIX_ACCOUNT_COUNT_V2)
            })
            .ok_or(CoreSbfError::Arithmetic)?;
        if (accounts.len() != bare && accounts.len() != gated)
            || funding_count == 0
            || funding_count > GENERIC_FOUNDING_MAX_FUNDING_STATES_V1
        {
            return Err(CoreSbfError::AccountFrame);
        }
        require_distinct(accounts)?;
        let found = ProjectedFoundAccountsV2::parse(
            program_id,
            accounts
                .get(..PROJECTED_FOUND_ACCOUNT_COUNT_V2)
                .ok_or(CoreSbfError::AccountFrame)?,
        )?;
        let trading_program = account(accounts, PROJECTED_FOUND_ACCOUNT_COUNT_V2)?;
        let trading_programdata = account(accounts, PROJECTED_FOUND_ACCOUNT_COUNT_V2 + 1)?;
        let funding_start = GENERIC_FOUNDING_FOUND_FIXED_ACCOUNT_COUNT_V1;
        let suffix_start = funding_start
            .checked_add(funding_count)
            .ok_or(CoreSbfError::Arithmetic)?;
        let funding = accounts
            .get(funding_start..suffix_start)
            .ok_or(CoreSbfError::AccountFrame)?;
        let suffix = GenericFoundSuffix::parse(
            accounts
                .get(suffix_start..)
                .ok_or(CoreSbfError::AccountFrame)?,
        )?;
        if trading_program.is_signer
            || trading_program.is_writable
            || !trading_program.executable
            || trading_programdata.is_signer
            || trading_programdata.is_writable
            || trading_programdata.executable
        {
            return Err(CoreSbfError::AccountFrame);
        }
        Ok(Self {
            found,
            trading_program,
            trading_programdata,
            funding,
            suffix,
        })
    }
}

impl<'accounts, 'info> GenericFoundSuffix<'accounts, 'info> {
    fn parse(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, CoreSbfError> {
        let gated = accounts.len() == GENERIC_FOUNDING_FOUND_PRICE_GATE_SUFFIX_ACCOUNT_COUNT_V2;
        if accounts.len() != GENERIC_FOUNDING_FOUND_SUFFIX_ACCOUNT_COUNT_V1 && !gated {
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
            price_gate: if gated {
                Some(FinalizedRecordFrameV2 {
                    raw: account(accounts, 15)?,
                    staging: account(accounts, 16)?,
                })
            } else {
                None
            },
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
        if let Some(price_gate) = value.price_gate {
            for readonly in [price_gate.raw, price_gate.staging] {
                if readonly.is_signer || readonly.is_writable || readonly.executable {
                    return Err(CoreSbfError::AccountFrame);
                }
            }
        }
        Ok(value)
    }
}

struct GenericOpenFrame<'accounts, 'info> {
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
    custody_replay: &'accounts AccountInfo<'info>,
    hoard: &'accounts AccountInfo<'info>,
    funding_source: &'accounts AccountInfo<'info>,
    aggregate: &'accounts AccountInfo<'info>,
    position: &'accounts AccountInfo<'info>,
    admission: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> GenericOpenFrame<'accounts, 'info> {
    fn parse(
        program_id: &Pubkey,
        accounts: &'accounts [AccountInfo<'info>],
    ) -> Result<Self, CoreSbfError> {
        if accounts.len() != GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1 {
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
            custody_replay: account(accounts, 15)?,
            hoard: account(accounts, 16)?,
            funding_source: account(accounts, 17)?,
            aggregate: account(accounts, 18)?,
            position: account(accounts, 19)?,
            admission: account(accounts, 20)?,
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
            || value.core_program.key != program_id
        {
            return Err(CoreSbfError::AccountFrame);
        }
        for program in [
            value.rent_program,
            value.registry_program,
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

    fn common(&self, capability_root: [u8; 32]) -> GenericFoundingOpenAccounts<'_, 'info> {
        GenericFoundingOpenAccounts {
            market: self.market,
            permit: self.permit,
            rent_credit: self.rent_credit,
            rent_program: self.rent_program,
            trading_program: self.trading_program,
            claims_program: self.claims_program,
            custody_program: self.custody_program,
            capability_root,
            custody_replay: self.custody_replay,
            hoard: self.hoard,
            funding_source: self.funding_source,
            aggregate: self.aggregate,
            position: self.position,
            admission: self.admission,
        }
    }
}

#[derive(Clone, Copy)]
struct GenericProductFacts {
    linked_basis_record: [u8; 32],
    semantic_basis: [u8; 32],
    claim_count: u32,
    basis_scale: u64,
}

#[derive(Clone, Copy)]
struct GenericProjectedFacts {
    realize_request_digest: [u8; 32],
    realize_receipt_digest: [u8; 32],
    realize_revision: u64,
    hoard_amount: u64,
}

/// Execute one externally selected generic founding stage.
#[inline(never)]
pub(crate) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: GenericFoundingRequestV1,
    request_bytes: &[u8],
    dependency_bytes: &[u8],
) -> Result<(), solana_program::program_error::ProgramError> {
    match request.stage() {
        GenericFoundingStageV1::FoundAndPermit => process_found(
            program_id,
            accounts,
            &request,
            request_bytes,
            dependency_bytes,
        ),
        GenericFoundingStageV1::Open => process_open(
            program_id,
            accounts,
            &request,
            request_bytes,
            dependency_bytes,
        ),
    }
}

#[inline(never)]
fn process_found(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: &GenericFoundingRequestV1,
    request_bytes: &[u8],
    lock_receipt_bytes: &[u8],
) -> Result<(), solana_program::program_error::ProgramError> {
    let frame =
        GenericFoundAccounts::parse(program_id, accounts, usize::from(request.funding_count()))?;
    let rent = Rent::get().map_err(|_| CoreSbfError::Creation)?;
    let clock = Clock::get().map_err(|_| CoreSbfError::Creation)?;
    if clock.slot > request.expiry_slot() {
        return Err(CoreSbfError::Reference.into());
    }
    let roles = authenticate_generic_found_roles(&frame, request)?;
    let prepared = prepare_generic_core_found(program_id, &frame, request, &rent, roles)?;
    authenticate_generic_core_found_references(
        &frame,
        request,
        request_bytes,
        &prepared,
        &rent,
        roles,
    )?;
    let permit_plan = prepare_generic_claims_permit(
        program_id,
        &frame,
        request,
        lock_receipt_bytes,
        &prepared,
        &rent,
    )?;
    found::apply_projected_prepared(program_id, &frame.found, *prepared)?;
    create_permit(
        program_id,
        frame.suffix.permit,
        frame.found.system,
        &permit_plan.permit,
        &rent,
    )?;
    finish_found(program_id, &frame, request, request_bytes)
}

#[inline(never)]
fn prepare_generic_core_found(
    program_id: &Pubkey,
    frame: &GenericFoundAccounts<'_, '_>,
    request: &GenericFoundingRequestV1,
    rent: &Rent,
    roles: RoleBatchAdmissions,
) -> Result<Box<PreparedFound>, solana_program::program_error::ProgramError> {
    roles.require(Role::Custody)?;
    let authority = authenticate_projected_found_authority(program_id, frame, request)?;
    MarketPrincipalCapSetsV1::read(authority.principal_cap_sets)
        .admit(request.quantity())
        .map_err(|_| CoreSbfError::Funding)?;
    let admission = roles.admission(Role::Core)?;
    let found_request = Request::administrative(
        Action::Found,
        request.generation(),
        identity(request.market().to_bytes())?,
    );
    let prepared = found::prepare_projected_with_admission(
        program_id,
        &frame.found,
        found_request,
        rent,
        admission,
        authority,
    )?;
    Ok(prepared)
}

#[inline(never)]
fn authenticate_projected_found_authority(
    program_id: &Pubkey,
    frame: &GenericFoundAccounts<'_, '_>,
    request: &GenericFoundingRequestV1,
) -> Result<ProjectedFoundAuthorityV2, CoreSbfError> {
    let data = frame
        .suffix
        .projected_replay
        .try_borrow_data()
        .map_err(|_| CoreSbfError::ChildAck)?;
    if frame.suffix.projected_replay.owner != frame.suffix.custody_program.key
        || data.len() != PROJECTED_CUSTODY_STATE_BYTES_V2
    {
        return Err(CoreSbfError::ChildAck);
    }
    let projected = decode_projected_state(&data)?;
    drop(data);
    let (expected, bump) = Pubkey::find_program_address(
        &ProjectedCustodyStateSeedsV2::from_request(projected.request).as_slices(),
        frame.suffix.custody_program.key,
    );
    let context = hashv(&[
        PROJECTED_HOARD_CONTEXT_DOMAIN_V1,
        request.context().to_bytes().as_slice(),
    ])
    .to_bytes();
    let principal = request
        .hoard_principal()
        .map_err(|_| CoreSbfError::Arithmetic)?;
    if expected != *frame.suffix.projected_replay.key
        || bump != projected.bump
        || !PROJECTED_CUSTODY_LOCKED_ADMISSIBLE_STATES_V1.admits(projected.phase)
        || projected.request.market != request.market().to_bytes()
        || projected.request.generation != request.generation()
        || projected.request.release_set != request.release_set().to_bytes()
        || projected.request.parent_capability_root != request.capability_root().to_bytes()
        || projected.request.context_digest != context
        || projected.request.caller_program != frame.trading_program.key.to_bytes()
        || projected.request.core_program != program_id.to_bytes()
        || projected.request.rent_program != frame.found.rent_program.key.to_bytes()
        || projected.request.refund_owner != request.beneficiary().to_bytes()
        || projected.request.rent_credit != frame.found.rent_credit.key.to_bytes()
        || projected.request.hoard_vault != request.hoard().to_bytes()
        || projected.request.funding_source_vault != request.funding_source().to_bytes()
        || projected.request.funding_source_context != request.context().to_bytes()
        || projected.request.expiry_slot != request.expiry_slot()
        || projected.locked_amount != principal
        || projected.principal_cap_sets == 0
    {
        return Err(CoreSbfError::ChildAck);
    }
    Ok(ProjectedFoundAuthorityV2 {
        realm_id: projected.request.realm,
        collateral_mint: projected.request.mint,
        token_program: projected.request.token_program,
        collateral_release: projected.request.collateral_release,
        resolution_policy_id: projected.request.source,
        principal_cap_sets: projected.principal_cap_sets,
    })
}

#[inline(never)]
fn authenticate_generic_core_found_references(
    frame: &GenericFoundAccounts<'_, '_>,
    request: &GenericFoundingRequestV1,
    request_bytes: &[u8],
    prepared: &PreparedFound,
    rent: &Rent,
    roles: RoleBatchAdmissions,
) -> Result<(), CoreSbfError> {
    authenticate_found_request(frame, request, request_bytes, prepared, rent)?;
    authenticate_generic_funding_and_capability_root(frame, request, prepared, roles)
}

#[inline(never)]
fn prepare_generic_claims_permit(
    program_id: &Pubkey,
    frame: &GenericFoundAccounts<'_, '_>,
    request: &GenericFoundingRequestV1,
    lock_receipt_bytes: &[u8],
    prepared: &PreparedFound,
    rent: &Rent,
) -> Result<GenericFoundingPermitPlanV1, CoreSbfError> {
    let lock_receipt = decode_lock_receipt(lock_receipt_bytes)?;
    let product = authenticate_generic_product(frame, prepared, rent)?;
    let projected =
        authenticate_generic_projected(program_id, frame, request, prepared, &lock_receipt)?;
    build_generic_found_permit(
        program_id,
        frame,
        request,
        prepared,
        &lock_receipt,
        lock_receipt_bytes,
        &product,
        &projected,
        rent,
    )
}

#[inline(never)]
fn finish_found(
    program_id: &Pubkey,
    frame: &GenericFoundAccounts<'_, '_>,
    request: &GenericFoundingRequestV1,
    request_bytes: &[u8],
) -> Result<(), solana_program::program_error::ProgramError> {
    let market_data = frame
        .found
        .market
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Commit)?;
    let permit_data = frame
        .suffix
        .permit
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Commit)?;
    let post = hashv(&[
        GENERIC_FOUNDING_FOUND_POST_RESOURCE_DOMAIN_V1,
        &market_data,
        &permit_data,
    ])
    .to_bytes();
    drop(market_data);
    drop(permit_data);
    return_ack(
        program_id,
        request,
        request_bytes,
        frame.suffix.permit.key,
        post,
    )
}

#[inline(never)]
fn process_open(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: &GenericFoundingRequestV1,
    request_bytes: &[u8],
    claims_receipt_bytes: &[u8],
) -> Result<(), solana_program::program_error::ProgramError> {
    let frame = GenericOpenFrame::parse(program_id, accounts)?;
    let rent = Rent::get().map_err(|_| CoreSbfError::Creation)?;
    let clock = Clock::get().map_err(|_| CoreSbfError::Creation)?;
    let roles = authenticate_generic_open_roles(&frame, request)?;
    let mut state = authenticate_generic_market(program_id, &frame, request)?;
    authenticate_generic_caller(frame.caller, frame.trading_program, request, request_bytes)?;
    // Decision 0004: the Found stage derived this address from the Market's own
    // authenticated capability manifest and Core persisted it in the Core-owned
    // permit. `authenticate_permit` requires the permit's `parent_root` to equal
    // it and `authenticate_open_request` requires the request to carry it, so an
    // Open stage that disagreed with its own Found stage cannot be constructed.
    let common = frame.common(request.capability_root().to_bytes());
    let permit = authenticate_permit(
        program_id,
        &common,
        request.context().to_bytes(),
        request.founder().to_bytes(),
        &rent,
        clock.slot,
        *state,
    )?;
    authenticate_open_request(request, &permit, frame.market.lamports())?;
    authenticate_rent_credit(
        &common,
        request.beneficiary().to_bytes(),
        request.release_set().to_bytes(),
        request.generation(),
        &rent,
    )?;
    let claims = decode_claims_receipt(claims_receipt_bytes)?;
    authenticate_claims_and_custody(&common, &permit, &claims, *state)?;
    apply_open(
        &mut state,
        &claims,
        roles.admission(Role::Claims)?,
        roles.admission(Role::Custody)?,
    )?;
    commit_market(frame.market, *state, program_id)?;
    close_permit(frame.permit, frame.rent_credit, program_id)?;
    let market_data = frame
        .market
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Commit)?;
    let post = hashv(&[
        GENERIC_FOUNDING_OPEN_POST_RESOURCE_DOMAIN_V1,
        &market_data,
        claims_receipt_bytes,
    ])
    .to_bytes();
    drop(market_data);
    if frame.permit.owner != &system_program::ID
        || frame.permit.lamports() != 0
        || !frame.permit.data_is_empty()
    {
        return Err(CoreSbfError::Commit.into());
    }
    return_ack(program_id, request, request_bytes, frame.permit.key, post)
}

#[inline(never)]
fn decode_lock_receipt(bytes: &[u8]) -> Result<Box<ProjectedCustodyLockReceiptV1>, CoreSbfError> {
    ProjectedCustodyLockReceiptV1::decode(bytes)
        .map(Box::new)
        .map_err(|_| CoreSbfError::ChildAck)
}

fn authenticate_generic_found_roles(
    frame: &GenericFoundAccounts<'_, '_>,
    request: &GenericFoundingRequestV1,
) -> Result<RoleBatchAdmissions, CoreSbfError> {
    authenticate_roles(
        frame.found.activation_cache,
        frame.found.registry_program,
        identity(frame.found.registry_program.key.to_bytes())?,
        request.release_set().to_bytes(),
        &[
            RoleDeploymentAccounts::new(
                Role::Core,
                frame.found.core_program,
                frame.found.core_programdata,
            ),
            RoleDeploymentAccounts::new(
                Role::Claims,
                frame.suffix.claims_program,
                frame.suffix.claims_programdata,
            ),
            RoleDeploymentAccounts::new(
                Role::Trading,
                frame.trading_program,
                frame.trading_programdata,
            ),
            RoleDeploymentAccounts::new(
                Role::Custody,
                frame.suffix.custody_program,
                frame.suffix.custody_programdata,
            ),
        ],
    )
}

fn authenticate_generic_open_roles(
    frame: &GenericOpenFrame<'_, '_>,
    request: &GenericFoundingRequestV1,
) -> Result<RoleBatchAdmissions, CoreSbfError> {
    authenticate_roles(
        frame.activation_cache,
        frame.registry_program,
        identity(frame.registry_program.key.to_bytes())?,
        request.release_set().to_bytes(),
        &[
            RoleDeploymentAccounts::new(Role::Core, frame.core_program, frame.core_programdata),
            RoleDeploymentAccounts::new(
                Role::Claims,
                frame.claims_program,
                frame.claims_programdata,
            ),
            RoleDeploymentAccounts::new(
                Role::Trading,
                frame.trading_program,
                frame.trading_programdata,
            ),
            RoleDeploymentAccounts::new(
                Role::Custody,
                frame.custody_program,
                frame.custody_programdata,
            ),
        ],
    )
}

/// Derive the sole founding capability root and require the request to name it.
///
/// Decision 0004. No root account exists while a Market is being founded: the
/// only route that creates one requires the Market to already be `Open`, which
/// is the last thing this same transaction does. Core therefore rebuilds
/// `CapabilityRootHeaderV1` from facts it has already authenticated and
/// derives the address under the Trading program.
///
/// This is strictly stronger than reading the account was. Every header field
/// is a function of the request plus the Market's capability manifest, so the
/// account never proved anything the request had not already fixed — while
/// `manifest`, `entry_index`, `kind`, and `capability_release` were supplied
/// entirely by the founder and checked only for self-consistency. Nothing
/// bound the selection to the Market's own authenticated manifest. Here the
/// manifest identity is the one Found authenticated, and the kind and release
/// come from that manifest's own indexed entry, exactly as Decision 0003
/// requires of the ordinary route.
fn authenticate_derived_capability_root(
    trading_program: &AccountInfo<'_>,
    request: &GenericFoundingRequestV1,
    manifest: CapabilityManifestV1<'_>,
    manifest_id: CapabilityContentId,
) -> Result<(), CoreSbfError> {
    // The authenticated manifest's actual entry count is the sole bound on the
    // index; `entry` refuses anything at or past it.
    let entry = manifest
        .entry(request.capability_entry_index())
        .map_err(|_| CoreSbfError::Reference)?;
    // The root PDA commits to the selected config identity, so the selected
    // config identity may not commit back to the root address. The codec owns
    // the sole root-free selection preimage; hashing the whole request here
    // would demand a SHA-256 fixed point and refuse every honest founder.
    let selected = request
        .selection_preimage()
        .map_err(|_| CoreSbfError::Instruction)?;
    let selection = CapabilityExecutionSelectionV1::from_bytes(
        request.capability_entry_index(),
        manifest_id.to_bytes(),
        entry.kind_id().to_bytes(),
        entry.release_id().to_bytes(),
        hash(&selected).to_bytes(),
    )
    .map_err(|_| CoreSbfError::Reference)?;
    let header = CapabilityRootHeaderV1::new(
        CapabilityContentId::new(request.release_set().to_bytes())
            .map_err(|_| CoreSbfError::Reference)?,
        request.market().to_bytes(),
        request.generation(),
        selection,
        // Core builds this header only to DERIVE the root address, never to
        // write one: the root PDA seeds are the semantic coordinates alone, so
        // the record bumps Trading fills in at activation are not among them
        // and no value here can move the address checked below.
        SelectedRecordBumpsV1::default(),
    )
    .map_err(|_| CoreSbfError::Reference)?;
    if Pubkey::find_program_address(&header.seeds().as_slices(), trading_program.key)
        .0
        .to_bytes()
        != request.capability_root().to_bytes()
    {
        return Err(CoreSbfError::Reference);
    }
    Ok(())
}

fn authenticate_generic_caller(
    caller: &AccountInfo<'_>,
    trading_program: &AccountInfo<'_>,
    request: &GenericFoundingRequestV1,
    request_bytes: &[u8],
) -> Result<(), CoreSbfError> {
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        request.release_set().to_bytes(),
        request.market().to_bytes(),
        ExecutionRoleV1::Trading,
        request.context().to_bytes(),
        hash(request_bytes).to_bytes(),
    )
    .map_err(|_| CoreSbfError::CallerAuthority)?;
    if !caller.is_signer
        || caller.executable
        || Pubkey::find_program_address(&seeds.as_slices(), trading_program.key).0 != *caller.key
    {
        return Err(CoreSbfError::CallerAuthority);
    }
    Ok(())
}

fn authenticate_found_request(
    frame: &GenericFoundAccounts<'_, '_>,
    request: &GenericFoundingRequestV1,
    request_bytes: &[u8],
    prepared: &PreparedFound,
    rent: &Rent,
) -> Result<(), CoreSbfError> {
    authenticate_generic_caller(
        frame.found.payer,
        frame.trading_program,
        request,
        request_bytes,
    )?;
    if prepared.release_set_id != request.release_set().to_bytes()
        || frame.found.market.key.to_bytes() != request.market().to_bytes()
        || frame.found.market.owner != &system_program::ID
        || frame.found.market.data_len() != 0
        || frame.found.market.lamports() != request.market_rent()
        || request.market_rent() != rent.minimum_balance(STATE_BYTES)
        || frame.suffix.permit.lamports() != request.permit_rent()
        || request.permit_rent() != rent.minimum_balance(SERIES_FOUNDING_PERMIT_BYTES_V1)
        || frame.suffix.founder.key.to_bytes() != request.founder().to_bytes()
        || frame.suffix.funding_source.key.to_bytes() != request.funding_source().to_bytes()
        || frame.suffix.hoard.key.to_bytes() != request.hoard().to_bytes()
        || frame.suffix.projected_replay.key.to_bytes() != request.projected_replay().to_bytes()
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
        || credit.market().to_bytes() != request.market().to_bytes()
        || credit.release_set().to_bytes() != request.release_set().to_bytes()
        || credit.generation() != request.generation()
    {
        return Err(CoreSbfError::RentCredit);
    }
    Ok(())
}

/// Authenticate the ordered FundingState span and the derived capability root.
///
/// Both consume the Market-selected capability manifest, so they share its one
/// decode. Splitting them would buy nothing and pay for a second decode inside
/// the founding transaction's compute budget.
fn authenticate_generic_funding_and_capability_root(
    frame: &GenericFoundAccounts<'_, '_>,
    request: &GenericFoundingRequestV1,
    prepared: &PreparedFound,
    roles: RoleBatchAdmissions,
) -> Result<(), CoreSbfError> {
    let manifest_data = frame
        .found
        .manifest_raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Funding)?;
    let manifest =
        CapabilityManifestV1::decode(&manifest_data).map_err(|_| CoreSbfError::Funding)?;
    if usize::from(request.funding_count()) != frame.funding.len() {
        return Err(CoreSbfError::Funding);
    }
    let manifest_id =
        CapabilityContentId::new(prepared.manifest_id).map_err(|_| CoreSbfError::Funding)?;
    authenticate_derived_capability_root(frame.trading_program, request, manifest, manifest_id)?;
    let resolution = roles.projected_binding(Role::Resolution);
    let trading = roles.projected_binding(Role::Trading);
    let mut expected_masks = founding_controller_masks(
        manifest,
        resolution.semantic_release.to_bytes(),
        request.capability_entry_index(),
    )?;
    let mut expected_controllers = [resolution.program, trading.program];
    if expected_masks[0].trailing_zeros() > expected_masks[1].trailing_zeros() {
        expected_masks.swap(0, 1);
        expected_controllers.swap(0, 1);
    }
    if frame.funding.len() != expected_masks.len() {
        return Err(CoreSbfError::Funding);
    }
    let required_union = if manifest.entry_count() == 16 {
        u16::MAX
    } else {
        1_u16
            .checked_shl(u32::from(manifest.entry_count()))
            .and_then(|value| value.checked_sub(1))
            .ok_or(CoreSbfError::Arithmetic)?
    };
    validate_funding_ledger_masks_v2(manifest.entry_count(), required_union, &expected_masks)
        .map_err(|_| CoreSbfError::Funding)?;

    let mut keys = [Identity::new([1; 32]).map_err(|_| CoreSbfError::Funding)?;
        GENERIC_FOUNDING_MAX_FUNDING_STATES_V1];
    for (index, funding_account) in frame.funding.iter().enumerate() {
        let controller = expected_controllers
            .get(index)
            .ok_or(CoreSbfError::Funding)?;
        let mask = *expected_masks.get(index).ok_or(CoreSbfError::Funding)?;
        if funding_account.owner.to_bytes() != controller.to_bytes()
            || funding_account.is_signer
            || funding_account.is_writable
            || funding_account.executable
        {
            return Err(CoreSbfError::Funding);
        }
        let data = funding_account
            .try_borrow_data()
            .map_err(|_| CoreSbfError::Funding)?;
        let funding = FundingLedgerV2::decode(&data).map_err(|_| CoreSbfError::Funding)?;
        if funding.selected_mask() != mask
            || funding.as_bytes().len()
                != funding_ledger_bytes_v2(funding.slot_count())
                    .map_err(|_| CoreSbfError::Funding)?
        {
            return Err(CoreSbfError::Funding);
        }
        let authenticated = funding
            .authenticate(manifest_id, manifest)
            .map_err(|_| CoreSbfError::Funding)?;
        let mut selected_entry = 0_u16;
        while selected_entry < manifest.entry_count() {
            if mask & (1_u16 << selected_entry) != 0 {
                let slot = authenticated
                    .slot(selected_entry)
                    .map_err(|_| CoreSbfError::Funding)?;
                if slot.status() != FundingLedgerStatusV2::Pending {
                    return Err(CoreSbfError::Funding);
                }
            }
            selected_entry = selected_entry
                .checked_add(1)
                .ok_or(CoreSbfError::Arithmetic)?;
        }
        authenticated
            .validate_recorded_native_custody(funding_account.lamports(), data.len(), false)
            .map_err(|error| match error {
                dclutch_capability_contract::Error::FundedRentNotEvidenced
                | dclutch_capability_contract::Error::FundedRentRateMissing => {
                    CoreSbfError::FundedRent
                }
                _ => CoreSbfError::Funding,
            })?;
        let derivation = CapabilityFundingLedgerDerivationV2::new(
            controller.to_bytes(),
            request.market().to_bytes(),
            request.generation(),
            manifest_id,
            funding,
        )
        .map_err(|_| CoreSbfError::Funding)?;
        let controller_program = Pubkey::new_from_array(controller.to_bytes());
        if Pubkey::find_program_address(&derivation.seed_components(), &controller_program).0
            != *funding_account.key
        {
            return Err(CoreSbfError::Funding);
        }
        *keys.get_mut(index).ok_or(CoreSbfError::Arithmetic)? =
            identity(funding_account.key.to_bytes())?;
    }
    let list = generic_founding_funding_list_id_v1(
        keys.get(..frame.funding.len())
            .ok_or(CoreSbfError::Arithmetic)?,
    )
    .map_err(|_| CoreSbfError::Funding)?;
    if list != request.funding_list_id() {
        return Err(CoreSbfError::Funding);
    }
    Ok(())
}

/// Derive the two controller-owned funding subsets from the authenticated
/// founding selection rather than from physical manifest positions.
///
/// The selected capability release is content-addressed family data and is
/// deliberately distinct from the generic Trading interpreter semantic
/// release (decision 0003). Every companion entry must name the exact activated
/// Resolution release. This generation's direct-capable founding is exactly
/// one selected Trading entry plus three Resolution companions.
fn founding_controller_masks(
    manifest: CapabilityManifestV1<'_>,
    resolution_semantic_release: [u8; 32],
    trading_entry_index: u16,
) -> Result<[u16; 2], CoreSbfError> {
    let trading_entry = manifest
        .entry(trading_entry_index)
        .map_err(|_| CoreSbfError::Funding)?;
    if trading_entry.release_id().to_bytes() == resolution_semantic_release {
        return Err(CoreSbfError::Funding);
    }
    let trading_mask = 1_u16
        .checked_shl(u32::from(trading_entry_index))
        .ok_or(CoreSbfError::Arithmetic)?;
    let mut resolution_mask = 0_u16;
    let mut entry_index = 0_u16;
    while entry_index < manifest.entry_count() {
        if entry_index != trading_entry_index {
            let entry = manifest
                .entry(entry_index)
                .map_err(|_| CoreSbfError::Funding)?;
            if entry.release_id().to_bytes() != resolution_semantic_release {
                return Err(CoreSbfError::Funding);
            }
            resolution_mask |= 1_u16
                .checked_shl(u32::from(entry_index))
                .ok_or(CoreSbfError::Arithmetic)?;
        }
        entry_index = entry_index.checked_add(1).ok_or(CoreSbfError::Arithmetic)?;
    }
    if manifest.entry_count() != 4 || resolution_mask.count_ones() != 3 {
        return Err(CoreSbfError::Funding);
    }
    Ok([resolution_mask, trading_mask])
}

fn authenticate_generic_product(
    frame: &GenericFoundAccounts<'_, '_>,
    prepared: &PreparedFound,
    rent: &Rent,
) -> Result<GenericProductFacts, CoreSbfError> {
    let product = authenticate_founding_product_basis_v3(
        frame.found.registry_program.key,
        rent,
        *prepared.runtime,
        FinalizedRecordFrameV2 {
            raw: frame.suffix.linked_basis_raw,
            staging: frame.suffix.linked_basis_staging,
        },
        frame.suffix.price_gate,
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
    Ok(GenericProductFacts {
        linked_basis_record: product.linked_basis_record.content_digest.to_bytes(),
        semantic_basis: product.semantic_basis_id.to_bytes(),
        claim_count: product.runtime.outcome_count,
        basis_scale: product.payout_scale,
    })
}

fn authenticate_generic_projected(
    program_id: &Pubkey,
    frame: &GenericFoundAccounts<'_, '_>,
    request: &GenericFoundingRequestV1,
    prepared: &PreparedFound,
    lock_receipt: &ProjectedCustodyLockReceiptV1,
) -> Result<GenericProjectedFacts, CoreSbfError> {
    let data = frame
        .suffix
        .projected_replay
        .try_borrow_data()
        .map_err(|_| CoreSbfError::ChildAck)?;
    if frame.suffix.projected_replay.owner != frame.suffix.custody_program.key
        || data.len() != PROJECTED_CUSTODY_STATE_BYTES_V2
    {
        return Err(CoreSbfError::ChildAck);
    }
    let projected = decode_projected_state(&data)?;
    drop(data);
    let (expected, bump) = Pubkey::find_program_address(
        &ProjectedCustodyStateSeedsV2::from_request(projected.request).as_slices(),
        frame.suffix.custody_program.key,
    );
    let context = hashv(&[
        PROJECTED_HOARD_CONTEXT_DOMAIN_V1,
        request.context().to_bytes().as_slice(),
    ])
    .to_bytes();
    let principal = request
        .hoard_principal()
        .map_err(|_| CoreSbfError::Arithmetic)?;
    if expected != *frame.suffix.projected_replay.key
        || bump != projected.bump
        || !PROJECTED_CUSTODY_LOCKED_ADMISSIBLE_STATES_V1.admits(projected.phase)
        || projected.request.market != request.market().to_bytes()
        || projected.request.generation != request.generation()
        || projected.request.realm != prepared.realm_id
        || projected.request.product_record != prepared.product_record_id
        || projected.request.product != prepared.product_id
        || projected.request.source != prepared.resolution_policy_id
        || projected.request.release_set != request.release_set().to_bytes()
        || projected.request.parent_capability_root != request.capability_root().to_bytes()
        || projected.request.context_digest != context
        || projected.request.caller_program != frame.trading_program.key.to_bytes()
        || projected.request.core_program != program_id.to_bytes()
        || projected.request.rent_program != frame.found.rent_program.key.to_bytes()
        || projected.request.refund_owner != request.beneficiary().to_bytes()
        || projected.request.rent_credit != frame.found.rent_credit.key.to_bytes()
        || projected.request.hoard_vault != request.hoard().to_bytes()
        || projected.request.funding_source_vault != request.funding_source().to_bytes()
        || projected.request.funding_source_context != request.context().to_bytes()
        || projected.request.mint != prepared.collateral_mint
        || projected.request.token_program != prepared.token_program
        || projected.request.collateral_release != prepared.collateral_release
        || projected.request.expiry_slot != request.expiry_slot()
        || projected.principal_cap_sets != prepared.candidate_state().principal_cap_sets
        || projected.locked_amount != principal
        || projected.last_request_digest != lock_receipt.request_digest
        || projected.next_revision != lock_receipt.resulting_revision
        || lock_receipt.market != request.market().to_bytes()
        || lock_receipt.release_set != request.release_set().to_bytes()
        || lock_receipt.context_digest != context
        || lock_receipt.source_vault != request.funding_source().to_bytes()
        || lock_receipt.source_replay != frame.suffix.funding_source_replay.key.to_bytes()
        || lock_receipt.hoard_vault != request.hoard().to_bytes()
        || lock_receipt.rent_credit != frame.found.rent_credit.key.to_bytes()
        || lock_receipt.amount != principal
        || lock_receipt.resulting_revision != projected.next_revision
    {
        return Err(CoreSbfError::ChildAck);
    }
    for closed in [
        frame.suffix.funding_source,
        frame.suffix.funding_source_replay,
    ] {
        if closed.owner != &system_program::ID
            || closed.lamports() != 0
            || !closed.data_is_empty()
            || closed.executable
        {
            return Err(CoreSbfError::ChildAck);
        }
    }
    if TokenProgram::parse(frame.suffix.hoard.owner.to_bytes())
        .map_err(|_| CoreSbfError::ChildAck)?
        .program_id()
        != prepared.token_program
    {
        return Err(CoreSbfError::ChildAck);
    }
    let hoard_data = frame
        .suffix
        .hoard
        .try_borrow_data()
        .map_err(|_| CoreSbfError::ChildAck)?;
    let hoard = TokenAccount::parse(&hoard_data).map_err(|_| CoreSbfError::ChildAck)?;
    if hoard.mint != prepared.collateral_mint
        || hoard.amount != principal
        || hoard.state != AccountState::Initialized
        || !hoard.delegate.is_none()
        || hoard.delegated_amount != 0
        || !hoard.native_reserve.is_none()
        || !hoard.close_authority.is_none()
    {
        return Err(CoreSbfError::ChildAck);
    }
    let mut realize = Box::new(projected.request);
    realize.operation = ProjectedCustodyOperationV1::RealizeAndClose;
    realize.expected_revision = projected.next_revision;
    realize.resulting_revision = projected
        .next_revision
        .checked_add(1)
        .ok_or(CoreSbfError::Arithmetic)?;
    realize.amount = projected.locked_amount;
    let realize_request_digest = digest_projected_request(&realize)?;
    finish_projected_facts(
        &projected,
        &realize,
        realize_request_digest,
        prepared.candidate_state_ref(),
        principal,
        frame.found.rent_credit.key.to_bytes(),
        request.projected_resulting_revision(),
    )
}

#[inline(never)]
fn decode_projected_state(bytes: &[u8]) -> Result<Box<ProjectedCustodyStateV2>, CoreSbfError> {
    ProjectedCustodyStateV2::decode(bytes)
        .map(Box::new)
        .map_err(|_| CoreSbfError::ChildAck)
}

#[inline(never)]
fn digest_projected_request(
    request: &dclutch_custody_contract::ProjectedCustodyRequestV1,
) -> Result<[u8; 32], CoreSbfError> {
    Ok(hash(&request.encode().map_err(|_| CoreSbfError::ChildAck)?).to_bytes())
}

#[inline(never)]
fn digest_core_state(state: &CoreState) -> Result<[u8; 32], CoreSbfError> {
    let bytes = state.encode().map_err(|_| CoreSbfError::Transition)?;
    // THE STATE NO RECEIPT REPRODUCES.
    //
    // The projected Realize receipt carries only `sha256(CoreState)`, and the
    // state itself is written two stages later -- so when a founding refuses
    // because the supervisor's intent hashes differently, the coordinate that
    // moved can be inside this preimage and nothing on chain shows it. This
    // emits the bytes Core hashed, under a feature no shipped build sets, so
    // the supervisor's own candidate encoding can be diffed against them.
    #[cfg(feature = "core-founding-probe")]
    {
        solana_program::log::sol_log("core generic founding: probe, candidate CoreState preimage");
        solana_program::log::sol_log_data(&[&bytes]);
    }
    Ok(hash(&bytes).to_bytes())
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn finish_projected_facts(
    projected: &ProjectedCustodyStateV2,
    realize: &dclutch_custody_contract::ProjectedCustodyRequestV1,
    realize_request_digest: [u8; 32],
    market: &CoreState,
    principal: u64,
    rent_credit: [u8; 32],
    expected_revision: u64,
) -> Result<GenericProjectedFacts, CoreSbfError> {
    let market_digest = digest_core_state(market)?;
    let receipt = projected
        .realize_and_close_ref(
            realize,
            realize_request_digest,
            market,
            market_digest,
            principal,
            rent_credit,
        )
        .map_err(|_| CoreSbfError::ChildAck)?;
    if receipt.resulting_revision != expected_revision {
        return Err(CoreSbfError::ChildAck);
    }
    Ok(GenericProjectedFacts {
        realize_request_digest,
        realize_receipt_digest: digest_projected_receipt(&receipt)?,
        realize_revision: receipt.resulting_revision,
        hoard_amount: principal,
    })
}

#[inline(never)]
fn digest_projected_receipt(
    receipt: &dclutch_custody_contract::ProjectedCustodyReceiptV1,
) -> Result<[u8; 32], CoreSbfError> {
    Ok(hash(&receipt.encode().map_err(|_| CoreSbfError::ChildAck)?).to_bytes())
}

#[allow(clippy::too_many_arguments)]
fn build_generic_found_permit(
    program_id: &Pubkey,
    frame: &GenericFoundAccounts<'_, '_>,
    request: &GenericFoundingRequestV1,
    prepared: &PreparedFound,
    lock_receipt: &ProjectedCustodyLockReceiptV1,
    lock_receipt_bytes: &[u8],
    product: &GenericProductFacts,
    projected: &GenericProjectedFacts,
    rent: &Rent,
) -> Result<GenericFoundingPermitPlanV1, CoreSbfError> {
    if product.basis_scale != request.basis_scale() {
        return Err(CoreSbfError::Reference);
    }
    let permit_seeds = SeriesFoundingPermitSeedsV1::new(
        request.release_set(),
        request.market(),
        request.context(),
    );
    let (expected_permit, bump) =
        Pubkey::find_program_address(&permit_seeds.as_slices(), program_id);
    if frame.suffix.permit.key != &expected_permit
        || frame.suffix.permit.owner != &system_program::ID
        || !frame.suffix.permit.data_is_empty()
        || frame.suffix.permit.lamports() != request.permit_rent()
    {
        return Err(CoreSbfError::Creation);
    }
    let expected_aggregate = Pubkey::find_program_address(
        &ClaimsFoundingAggregateSeedsV5::new(request.market().to_bytes())
            .map_err(|_| CoreSbfError::Reference)?
            .as_slices(),
        frame.suffix.claims_program.key,
    )
    .0;
    let expected_position = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(expected_aggregate.to_bytes(), request.founder().to_bytes())
            .map_err(|_| CoreSbfError::Reference)?
            .as_slices(),
        frame.suffix.claims_program.key,
    )
    .0;
    let expected_admission = Pubkey::find_program_address(
        &ProtocolPositionAdmissionSeedsV2::new(
            expected_aggregate.to_bytes(),
            request.founder().to_bytes(),
        )
        .map_err(|_| CoreSbfError::Reference)?
        .as_slices(),
        frame.suffix.claims_program.key,
    )
    .0;
    if frame.suffix.aggregate.key != &expected_aggregate
        || frame.suffix.position.key != &expected_position
        || frame.suffix.admission.key != &expected_admission
    {
        return Err(CoreSbfError::Reference);
    }
    for vacant in [
        frame.suffix.aggregate,
        frame.suffix.position,
        frame.suffix.admission,
    ] {
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
    build_permit_plan(GenericFoundingPermitInputV1 {
        bump,
        release_set: request.release_set().to_bytes(),
        market: request.market().to_bytes(),
        product_record: prepared.product_record_id,
        product_id: prepared.product_id,
        linked_basis_record: product.linked_basis_record,
        semantic_basis: product.semantic_basis,
        source: prepared.resolution_policy_id,
        founder: request.founder().to_bytes(),
        context: request.context().to_bytes(),
        capability_root: request.capability_root().to_bytes(),
        projected_replay: request.projected_replay().to_bytes(),
        funding_source: request.funding_source().to_bytes(),
        hoard: request.hoard().to_bytes(),
        projected_request_digest: projected.realize_request_digest,
        projected_receipt_digest: projected.realize_receipt_digest,
        custody_lock_request_digest: lock_receipt.request_digest,
        custody_lock_receipt_digest: hash(lock_receipt_bytes).to_bytes(),
        trading_program: frame.trading_program.key.to_bytes(),
        claims_program: frame.suffix.claims_program.key.to_bytes(),
        rent_credit: frame.found.rent_credit.key.to_bytes(),
        rent_program: frame.found.rent_program.key.to_bytes(),
        aggregate: frame.suffix.aggregate.key.to_bytes(),
        position: frame.suffix.position.key.to_bytes(),
        admission: frame.suffix.admission.key.to_bytes(),
        generation: request.generation(),
        claim_count: product.claim_count,
        quantity: request.quantity(),
        basis_scale: request.basis_scale(),
        expiry_slot: request.expiry_slot(),
        projected_resulting_revision: projected.realize_revision,
        normal_replay_revision: 1,
        source_amount: request
            .hoard_principal()
            .map_err(|_| CoreSbfError::Arithmetic)?,
        hoard_amount: projected.hoard_amount,
        aggregate_rent: rent.minimum_balance(aggregate_width),
        position_rent: rent.minimum_balance(position_width),
        admission_rent: rent.minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2),
        aggregate_lamports: frame.suffix.aggregate.lamports(),
        position_lamports: frame.suffix.position.lamports(),
        admission_lamports: frame.suffix.admission.lamports(),
    })
}

fn authenticate_generic_market(
    program_id: &Pubkey,
    frame: &GenericOpenFrame<'_, '_>,
    request: &GenericFoundingRequestV1,
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
    if Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(state.identity).as_slices(),
        program_id,
    )
    .0 != *frame.market.key
        || state.identity.market_id.to_bytes() != request.market().to_bytes()
        || state.identity.registry_program.to_bytes() != frame.registry_program.key.to_bytes()
        || state.identity.selected_release_set.to_bytes() != request.release_set().to_bytes()
        || state.identity.generation != request.generation()
        || state.rent_beneficiary.to_bytes() != frame.rent_credit.key.to_bytes()
        || !GENERIC_FOUNDING_OPEN_ADMISSIBLE_PRESTATES_V1.admits(state.phase, state.readiness)
    {
        return Err(CoreSbfError::Market);
    }
    Ok(Box::new(state))
}

fn authenticate_open_request(
    request: &GenericFoundingRequestV1,
    permit: &SeriesFoundingPermitV1,
    market_lamports: u64,
) -> Result<(), CoreSbfError> {
    let intent = permit.intent();
    if intent.release_set().to_bytes() != request.release_set().to_bytes()
        || intent.market().to_bytes() != request.market().to_bytes()
        || intent.parent_root().to_bytes() != request.capability_root().to_bytes()
        || intent.ticket_context().to_bytes() != request.context().to_bytes()
        || intent.founder().to_bytes() != request.founder().to_bytes()
        || intent.funding_source().to_bytes() != request.funding_source().to_bytes()
        || intent.hoard().to_bytes() != request.hoard().to_bytes()
        || intent.projected_replay().to_bytes() != request.projected_replay().to_bytes()
        || intent.generation() != request.generation()
        || intent.quantity() != request.quantity()
        || intent.basis_scale() != request.basis_scale()
        || intent.expiry_slot() != request.expiry_slot()
        || intent.projected_resulting_revision() != request.projected_resulting_revision()
        || market_lamports < request.market_rent()
    {
        return Err(CoreSbfError::Reference);
    }
    Ok(())
}

fn return_ack(
    program_id: &Pubkey,
    request: &GenericFoundingRequestV1,
    request_bytes: &[u8],
    permit: &Pubkey,
    post_resource_digest: [u8; 32],
) -> Result<(), solana_program::program_error::ProgramError> {
    let ack = GenericFoundingAckV1::new(
        *request,
        identity(program_id.to_bytes())?,
        identity(permit.to_bytes())?,
        identity(hash(request_bytes).to_bytes())?,
        identity(post_resource_digest)?,
    )
    .encode()
    .map_err(|_| CoreSbfError::ChildAck)?;
    set_return_data(&ack);
    Ok(())
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, CoreSbfError> {
    accounts.get(index).ok_or(CoreSbfError::AccountFrame)
}

/// Fully authenticated inputs to the family-neutral Claims founding compiler.
pub(crate) struct GenericFoundingPermitInputV1 {
    pub(crate) bump: u8,
    pub(crate) release_set: [u8; 32],
    pub(crate) market: [u8; 32],
    pub(crate) product_record: [u8; 32],
    pub(crate) product_id: [u8; 32],
    pub(crate) linked_basis_record: [u8; 32],
    pub(crate) semantic_basis: [u8; 32],
    pub(crate) source: [u8; 32],
    pub(crate) founder: [u8; 32],
    pub(crate) context: [u8; 32],
    pub(crate) capability_root: [u8; 32],
    pub(crate) projected_replay: [u8; 32],
    pub(crate) funding_source: [u8; 32],
    pub(crate) hoard: [u8; 32],
    pub(crate) projected_request_digest: [u8; 32],
    pub(crate) projected_receipt_digest: [u8; 32],
    pub(crate) custody_lock_request_digest: [u8; 32],
    pub(crate) custody_lock_receipt_digest: [u8; 32],
    pub(crate) trading_program: [u8; 32],
    pub(crate) claims_program: [u8; 32],
    pub(crate) rent_credit: [u8; 32],
    pub(crate) rent_program: [u8; 32],
    pub(crate) aggregate: [u8; 32],
    pub(crate) position: [u8; 32],
    pub(crate) admission: [u8; 32],
    pub(crate) generation: u64,
    pub(crate) claim_count: u32,
    pub(crate) quantity: u64,
    pub(crate) basis_scale: u64,
    pub(crate) expiry_slot: u64,
    pub(crate) projected_resulting_revision: u64,
    pub(crate) normal_replay_revision: u64,
    pub(crate) source_amount: u64,
    pub(crate) hoard_amount: u64,
    pub(crate) aggregate_rent: u64,
    pub(crate) position_rent: u64,
    pub(crate) admission_rent: u64,
    pub(crate) aggregate_lamports: u64,
    pub(crate) position_lamports: u64,
    pub(crate) admission_lamports: u64,
}

/// One exact Claims request and matching Core-owned one-shot permit.
pub(crate) struct GenericFoundingPermitPlanV1 {
    pub(crate) permit: Box<SeriesFoundingPermitV1>,
}

/// Compile one Claims FoundingV5 request and permit from authenticated facts.
#[inline(never)]
pub(crate) fn build_permit_plan(
    input: GenericFoundingPermitInputV1,
) -> Result<GenericFoundingPermitPlanV1, CoreSbfError> {
    let intent = build_founding_intent(&input)?;
    let intent_digest = digest_founding_intent(&intent)?;
    let claims_digest = build_claims_request_digest(&input, intent_digest)?;
    finish_permit_plan(*intent, intent_digest, claims_digest)
}

#[inline(never)]
fn build_founding_intent(
    input: &GenericFoundingPermitInputV1,
) -> Result<Box<FoundingIntentV5>, CoreSbfError> {
    FoundingIntentV5::new(
        input.bump,
        identity(input.release_set)?,
        identity(input.market)?,
        identity(input.product_record)?,
        identity(input.source)?,
        identity(input.founder)?,
        identity(input.context)?,
        identity(input.capability_root)?,
        identity(input.projected_replay)?,
        identity(input.funding_source)?,
        identity(input.hoard)?,
        identity(input.projected_request_digest)?,
        identity(input.projected_receipt_digest)?,
        identity(input.trading_program)?,
        identity(input.claims_program)?,
        identity(input.rent_credit)?,
        input.generation,
        input.quantity,
        input.basis_scale,
        input.expiry_slot,
        input.projected_resulting_revision,
        input.normal_replay_revision,
    )
    .map(Box::new)
    .map_err(|_| CoreSbfError::Reference)
}

#[inline(never)]
fn digest_founding_intent(intent: &FoundingIntentV5) -> Result<[u8; 32], CoreSbfError> {
    Ok(hash(&intent.encode().map_err(|_| CoreSbfError::Reference)?).to_bytes())
}

#[inline(never)]
fn build_claims_request_digest(
    input: &GenericFoundingPermitInputV1,
    intent_digest: [u8; 32],
) -> Result<[u8; 32], CoreSbfError> {
    let claims = ClaimsFoundingRequestV5::new(ClaimsFoundingRequestInputV5 {
        release_set: input.release_set,
        market: input.market,
        product_record_digest: input.product_record,
        product_instance_id: input.product_id,
        linked_basis_record_digest: input.linked_basis_record,
        semantic_basis_id: input.semantic_basis,
        founder: input.founder,
        founding_intent_digest: intent_digest,
        aggregate: input.aggregate,
        position: input.position,
        admission: input.admission,
        hoard: input.hoard,
        rent_credit: input.rent_credit,
        rent_program: input.rent_program,
        claims_program: input.claims_program,
        trading_program: input.trading_program,
        funding_source: input.funding_source,
        custody_replay: input.projected_replay,
        custody_request_digest: input.custody_lock_request_digest,
        custody_receipt_digest: input.custody_lock_receipt_digest,
        generation: input.generation,
        claim_count: input.claim_count,
        quantity: input.quantity,
        basis_scale: input.basis_scale,
        pre_source_amount: input.source_amount,
        post_source_amount: 0,
        pre_hoard_amount: 0,
        post_hoard_amount: input.hoard_amount,
        pre_custody_revision: 0,
        post_custody_revision: input.normal_replay_revision,
        aggregate_rent_principal: input.aggregate_rent,
        position_rent_principal: input.position_rent,
        admission_rent_principal: input.admission_rent,
        observed_aggregate_lamports: input.aggregate_lamports,
        observed_position_lamports: input.position_lamports,
        observed_admission_lamports: input.admission_lamports,
        pre_aggregate_revision: 0,
        post_aggregate_revision: 1,
        pre_position_revision: 0,
        post_position_revision: 1,
    })
    .map_err(|_| CoreSbfError::Reference)?;
    Ok(hash(&claims.to_bytes()).to_bytes())
}

#[inline(never)]
fn finish_permit_plan(
    intent: FoundingIntentV5,
    intent_digest: [u8; 32],
    claims_digest: [u8; 32],
) -> Result<GenericFoundingPermitPlanV1, CoreSbfError> {
    Ok(GenericFoundingPermitPlanV1 {
        permit: Box::new(
            SeriesFoundingPermitV1::new(intent, identity(intent_digest)?, identity(claims_digest)?)
                .map_err(|_| CoreSbfError::Reference)?,
        ),
    })
}

/// Allocate, assign, and commit the exact Core-owned one-shot permit.
#[inline(never)]
pub(crate) fn create_permit<'info>(
    program_id: &Pubkey,
    permit_account: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    permit: &SeriesFoundingPermitV1,
    rent: &Rent,
) -> Result<(), CoreSbfError> {
    if permit_account.owner != &system_program::ID
        || !permit_account.data_is_empty()
        || permit_account.lamports() < rent.minimum_balance(SERIES_FOUNDING_PERMIT_BYTES_V1)
    {
        return Err(CoreSbfError::Creation);
    }
    let seeds = permit.seeds();
    let base = seeds.as_slices();
    let bump = [permit.intent().bump()];
    let signer = [base[0], base[1], base[2], base[3], bump.as_slice()];
    for instruction in [
        allocate(
            permit_account.key,
            u64::try_from(SERIES_FOUNDING_PERMIT_BYTES_V1).map_err(|_| CoreSbfError::Arithmetic)?,
        ),
        assign(permit_account.key, program_id),
    ] {
        invoke_signed(
            &instruction,
            &[permit_account.clone(), system.clone()],
            &[&signer],
        )
        .map_err(|_| CoreSbfError::Creation)?;
    }
    let encoded = permit.encode().map_err(|_| CoreSbfError::Commit)?;
    permit_account
        .try_borrow_mut_data()
        .map_err(|_| CoreSbfError::Commit)?
        .copy_from_slice(&encoded);
    let data = permit_account
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Commit)?;
    if permit_account.owner != program_id || SeriesFoundingPermitV1::decode(&data) != Ok(*permit) {
        return Err(CoreSbfError::Commit);
    }
    Ok(())
}

/// Accounts whose semantics are identical for Series and generic founding.
pub(crate) struct GenericFoundingOpenAccounts<'accounts, 'info> {
    pub(crate) market: &'accounts AccountInfo<'info>,
    pub(crate) permit: &'accounts AccountInfo<'info>,
    pub(crate) rent_credit: &'accounts AccountInfo<'info>,
    pub(crate) rent_program: &'accounts AccountInfo<'info>,
    pub(crate) trading_program: &'accounts AccountInfo<'info>,
    pub(crate) claims_program: &'accounts AccountInfo<'info>,
    pub(crate) custody_program: &'accounts AccountInfo<'info>,
    /// Address of the sole Trading capability root for this Market generation.
    ///
    /// Decision 0004: no root account exists during generic founding, so this
    /// is the address the Found stage derived and the permit persisted. The
    /// Series route still passes its live root account's key.
    pub(crate) capability_root: [u8; 32],
    pub(crate) custody_replay: &'accounts AccountInfo<'info>,
    pub(crate) hoard: &'accounts AccountInfo<'info>,
    pub(crate) funding_source: &'accounts AccountInfo<'info>,
    pub(crate) aggregate: &'accounts AccountInfo<'info>,
    pub(crate) position: &'accounts AccountInfo<'info>,
    pub(crate) admission: &'accounts AccountInfo<'info>,
}

/// Authenticate the sole Core permit for one already family-admitted context.
///
/// Completion-only, per the 2026-08-29 13:30 founding ruling: an allocated
/// permit has no refund route and never expires, so the permissionless Open
/// stays available to everyone forever and stage-1-committed value cannot
/// strand. `expiry_slot` still gates the Found stage and the pre-allocation
/// refund family (`series_permit_expiry`), which only ever touches a permit
/// this route cannot accept (System-owned, data-empty). The slot argument is
/// retained unused so neither caller nor frame changes shape.
#[inline(never)]
pub(crate) fn authenticate_permit(
    program_id: &Pubkey,
    frame: &GenericFoundingOpenAccounts<'_, '_>,
    context: [u8; 32],
    founder: [u8; 32],
    rent: &Rent,
    _current_slot: u64,
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
    let (expected_permit, bump) =
        Pubkey::find_program_address(&permit.seeds().as_slices(), program_id);
    if expected_permit != *frame.permit.key
        || bump != intent.bump()
        || intent.market().to_bytes() != frame.market.key.to_bytes()
        || intent.release_set().to_bytes() != state.identity.selected_release_set.to_bytes()
        || intent.ticket_context().to_bytes() != context
        || intent.founder().to_bytes() != founder
        || intent.product_record().to_bytes() != state.identity.product_record.to_bytes()
        || intent.source().to_bytes() != state.identity.resolution_policy.to_bytes()
        || intent.parent_root().to_bytes() != frame.capability_root
        || intent.projected_replay().to_bytes() != frame.custody_replay.key.to_bytes()
        || intent.funding_source().to_bytes() != frame.funding_source.key.to_bytes()
        || intent.hoard().to_bytes() != frame.hoard.key.to_bytes()
        || intent.trading_program().to_bytes() != frame.trading_program.key.to_bytes()
        || intent.claims_program().to_bytes() != frame.claims_program.key.to_bytes()
        || intent.rent_credit().to_bytes() != frame.rent_credit.key.to_bytes()
        || intent.generation() != state.identity.generation
    {
        return Err(CoreSbfError::Reference);
    }
    Ok(Box::new(permit))
}

/// Decode the exact Claims receipt retained by the atomic Trading outer.
#[inline(never)]
pub(crate) fn decode_claims_receipt(
    claims_receipt_bytes: &[u8],
) -> Result<Box<ClaimsFoundingReceiptV5>, CoreSbfError> {
    ClaimsFoundingReceiptV5::decode(claims_receipt_bytes)
        .map(Box::new)
        .map_err(|_| CoreSbfError::ChildAck)
}

/// Authenticate Claims and Custody poststate against the one-shot permit.
#[inline(never)]
pub(crate) fn authenticate_claims_and_custody(
    frame: &GenericFoundingOpenAccounts<'_, '_>,
    permit: &SeriesFoundingPermitV1,
    receipt: &ClaimsFoundingReceiptV5,
    state: CoreState,
) -> Result<(), CoreSbfError> {
    let intent = permit.intent();
    let request = receipt.request();
    receipt
        .verify_for(&request, permit.claims_request_digest().to_bytes())
        .map_err(|_| CoreSbfError::ChildAck)?;
    let intent_digest = hash(&intent.encode().map_err(|_| CoreSbfError::ChildAck)?).to_bytes();
    permit
        .verify_for_intent_and_request(
            intent,
            identity(intent_digest)?,
            identity(receipt.request_digest())?,
        )
        .map_err(|_| CoreSbfError::ChildAck)?;
    if request.release_set() != state.identity.selected_release_set.to_bytes()
        || request.market() != frame.market.key.to_bytes()
        || request.product_record_digest() != state.identity.product_record.to_bytes()
        || request.product_instance_id() != state.identity.product_id.to_bytes()
        || request.founder() != intent.founder().to_bytes()
        || request.founding_intent_digest() != intent_digest
        || request.aggregate() != frame.aggregate.key.to_bytes()
        || request.position() != frame.position.key.to_bytes()
        || request.admission() != frame.admission.key.to_bytes()
        || request.funding_source() != frame.funding_source.key.to_bytes()
        || request.hoard() != frame.hoard.key.to_bytes()
        || request.custody_replay() != frame.custody_replay.key.to_bytes()
        || request.rent_credit() != frame.rent_credit.key.to_bytes()
        || request.rent_program() != frame.rent_program.key.to_bytes()
        || request.claims_program() != frame.claims_program.key.to_bytes()
        || request.trading_program() != frame.trading_program.key.to_bytes()
        || request.generation() != state.identity.generation
        || request.quantity() != intent.quantity()
        || request.basis_scale() != intent.basis_scale()
        || request.post_custody_revision() != intent.normal_replay_revision()
        || request.post_source_amount() != 0
        || request.pre_source_amount() != request.collateral_transferred()
        || request.post_hoard_amount() != request.collateral_transferred()
    {
        return Err(CoreSbfError::ChildAck);
    }
    authenticate_claims_poststate(frame, receipt)?;
    authenticate_custody_poststate(frame, request, intent)
}

/// Apply the sole checked Market Open transition after physical poststate.
#[inline(never)]
pub(crate) fn apply_open(
    state: &mut CoreState,
    receipt: &ClaimsFoundingReceiptV5,
    claims_admission: Admission,
    custody_admission: Admission,
) -> Result<(), CoreSbfError> {
    let request = receipt.request();
    open_series_market(
        Request::administrative(
            Action::OpenMarket,
            state.identity.generation,
            state.identity.market_id,
        ),
        state,
        SeriesOpenObservation {
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
            claims_effect: complete_child_effect(),
            custody_effect: complete_child_effect(),
        },
    )
    .map_err(|_| CoreSbfError::Transition)
}

/// Authenticate the permanent LifecycleRentCreditV2 before permit closure.
pub(crate) fn authenticate_rent_credit(
    frame: &GenericFoundingOpenAccounts<'_, '_>,
    beneficiary: [u8; 32],
    release_set: [u8; 32],
    generation: u64,
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
    if credit.refund_wallet().to_bytes() != beneficiary
        || credit.market().to_bytes() != frame.market.key.to_bytes()
        || credit.release_set().to_bytes() != release_set
        || credit.generation() != generation
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

/// Commit the authenticated candidate Market state.
pub(crate) fn commit_market(
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

/// Close the consumed one-shot permit only to the immutable RentCredit.
pub(crate) fn close_permit(
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

fn authenticate_claims_poststate(
    frame: &GenericFoundingOpenAccounts<'_, '_>,
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
    frame: &GenericFoundingOpenAccounts<'_, '_>,
    request: dclutch_claims_svm::founding_v5::ClaimsFoundingRequestV5,
    intent: dclutch_market_core_codec::FoundingIntentV5,
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
    let context = intent.ticket_context().to_bytes();
    let projected_context =
        hashv(&[PROJECTED_HOARD_CONTEXT_DOMAIN_V1, context.as_slice()]).to_bytes();
    // The realized replay is the TRADING compartment of the Market's namespace,
    // and the role is a seed. Composed through the owning type rather than
    // restated: a restatement here is exactly what would have kept deriving the
    // role-less address after the seeds moved, silently, with everything green.
    let replay_expected = Pubkey::find_program_address(
        &dclutch_custody_contract::CustodyReplaySeedsV1::new(
            request.market(),
            request.release_set(),
            CallerRoleV1::Trading,
            projected_context,
        )
        .as_slices(),
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

fn complete_child_effect() -> ChildEffectObservation {
    ChildEffectObservation {
        exact_request_authenticated: true,
        exact_receipt_authenticated: true,
        post_resource_authenticated: true,
    }
}

fn identity(bytes: [u8; 32]) -> Result<dclutch_market_core_codec::Identity, CoreSbfError> {
    dclutch_market_core_codec::Identity::new(bytes).map_err(|_| CoreSbfError::ChildAck)
}

#[cfg(test)]
mod tests {
    use alloc::{boxed::Box, vec::Vec};

    use dclutch_capability_contract::{
        ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CompartmentFundingV1,
        FundingAmountsV1, FundingQuoteV1, MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
    };

    use super::*;

    fn id(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn suffix_accounts(count: usize) -> Vec<AccountInfo<'static>> {
        (0..count)
            .map(|index| {
                let key = Box::leak(Box::new(Pubkey::new_from_array(
                    [u8::try_from(index + 1).expect("bounded test index"); 32],
                )));
                let owner = Box::leak(Box::new(Pubkey::new_from_array([0xa1; 32])));
                let lamports = Box::leak(Box::new(1_u64));
                let data = Box::leak(Vec::<u8>::new().into_boxed_slice());
                AccountInfo::new(
                    key,
                    false,
                    index == 0,
                    lamports,
                    data,
                    owner,
                    index == 7 || index == 9,
                )
            })
            .collect()
    }

    fn input() -> GenericFoundingPermitInputV1 {
        GenericFoundingPermitInputV1 {
            bump: 7,
            release_set: id(1),
            market: id(2),
            product_record: id(3),
            product_id: id(4),
            linked_basis_record: id(5),
            semantic_basis: id(6),
            source: id(7),
            founder: id(8),
            context: id(9),
            capability_root: id(10),
            projected_replay: id(11),
            funding_source: id(12),
            hoard: id(13),
            projected_request_digest: id(14),
            projected_receipt_digest: id(15),
            custody_lock_request_digest: id(16),
            custody_lock_receipt_digest: id(17),
            trading_program: id(18),
            claims_program: id(19),
            rent_credit: id(20),
            rent_program: id(21),
            aggregate: id(22),
            position: id(23),
            admission: id(24),
            generation: 1,
            claim_count: 3,
            quantity: 2,
            basis_scale: 5,
            expiry_slot: 100,
            projected_resulting_revision: 3,
            normal_replay_revision: 1,
            source_amount: 10,
            hoard_amount: 10,
            aggregate_rent: 100,
            position_rent: 100,
            admission_rent: 100,
            aggregate_lamports: 100,
            position_lamports: 100,
            admission_lamports: 100,
        }
    }

    fn manifest_with_foreign_entries(foreign: &[usize]) -> Vec<u8> {
        let native = CompartmentFundingV1::native_lamports(1).expect("native");
        let none = CompartmentFundingV1::not_applicable();
        let amounts =
            FundingAmountsV1::new(native, native, none, none, native, none, none).expect("amounts");
        let quote = FundingQuoteV1::new(amounts, None).expect("quote");
        let mut entries = Vec::new();
        for index in 0_usize..4 {
            let byte = u8::try_from(index).expect("bounded index");
            entries.push(
                CapabilityEntryV1::new(
                    CapabilityContentId::new([0x10 + byte; 32]).expect("kind"),
                    CapabilityContentId::new(if foreign.contains(&index) {
                        [0x31; 32]
                    } else {
                        [0x30; 32]
                    })
                    .expect("release"),
                    CapabilityContentId::new([0x40 + byte; 32]).expect("config"),
                    CapabilityContentId::new([0x50 + byte; 32]).expect("capacity"),
                    CapabilityContentId::new([0x60; 32]).expect("schema"),
                    CapabilityContentId::new([0x70; 32]).expect("derivation"),
                    ActivationPolicy::RequiredAtFounding,
                    0,
                    0,
                    [0; MAX_DEPENDENCIES_PER_CAPABILITY],
                    quote,
                )
                .expect("entry"),
            );
        }
        let mut bytes =
            alloc::vec![0_u8; MANIFEST_HEADER_BYTES + entries.len() * CAPABILITY_ENTRY_BYTES];
        CapabilityManifestV1::encode_into(&entries, &mut bytes).expect("manifest");
        bytes
    }

    #[test]
    fn founding_controller_masks_follow_the_authenticated_selected_entry() {
        for trading_index in 0_usize..4 {
            let bytes = manifest_with_foreign_entries(&[trading_index]);
            let manifest = CapabilityManifestV1::decode(&bytes).expect("manifest");
            let trading_mask = 1_u16 << trading_index;
            assert_eq!(
                founding_controller_masks(
                    manifest,
                    [0x30; 32],
                    u16::try_from(trading_index).expect("index"),
                ),
                Ok([0b1111 ^ trading_mask, trading_mask]),
            );
        }
    }

    #[test]
    fn founding_controller_masks_refuse_ambiguous_or_invalid_selection() {
        let all_resolution = manifest_with_foreign_entries(&[]);
        assert_eq!(
            founding_controller_masks(
                CapabilityManifestV1::decode(&all_resolution).expect("manifest"),
                [0x30; 32],
                0,
            ),
            Err(CoreSbfError::Funding),
        );

        let two_foreign = manifest_with_foreign_entries(&[1, 3]);
        assert_eq!(
            founding_controller_masks(
                CapabilityManifestV1::decode(&two_foreign).expect("manifest"),
                [0x30; 32],
                3,
            ),
            Err(CoreSbfError::Funding),
        );

        let one_foreign = manifest_with_foreign_entries(&[3]);
        assert_eq!(
            founding_controller_masks(
                CapabilityManifestV1::decode(&one_foreign).expect("manifest"),
                [0x30; 32],
                4,
            ),
            Err(CoreSbfError::Funding),
        );
    }

    #[test]
    fn permit_compiler_binds_exact_claims_request_and_refuses_conservation_drift() {
        let valid = input();
        let plan = build_permit_plan(valid).expect("permit");
        assert_eq!(plan.permit.intent().market().to_bytes(), id(2));
        assert_eq!(plan.permit.intent().quantity(), 2);

        let mut hostile = input();
        hostile.hoard_amount = 9;
        assert_eq!(
            build_permit_plan(hostile).err(),
            Some(CoreSbfError::Reference)
        );
    }

    #[test]
    fn generic_found_suffix_admits_only_bare_or_complete_appended_gate_pair() {
        let bare = suffix_accounts(GENERIC_FOUNDING_FOUND_SUFFIX_ACCOUNT_COUNT_V1);
        let bare = GenericFoundSuffix::parse(&bare).expect("legacy categorical suffix");
        assert!(bare.price_gate.is_none());

        let gated = suffix_accounts(GENERIC_FOUNDING_FOUND_PRICE_GATE_SUFFIX_ACCOUNT_COUNT_V2);
        let parsed = GenericFoundSuffix::parse(&gated).expect("complete gated suffix");
        let gate = parsed.price_gate.expect("gate pair");
        assert_eq!(gate.raw.key, gated[15].key);
        assert_eq!(gate.staging.key, gated[16].key);

        let partial = suffix_accounts(GENERIC_FOUNDING_FOUND_SUFFIX_ACCOUNT_COUNT_V1 + 1);
        assert_eq!(
            GenericFoundSuffix::parse(&partial).err(),
            Some(CoreSbfError::AccountFrame)
        );
    }
}
