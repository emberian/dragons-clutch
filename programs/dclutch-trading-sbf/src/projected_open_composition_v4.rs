//! Exact final Core Open and Trading replay commit for projected Markets.

extern crate alloc;

use alloc::vec::Vec;

use dclutch_claims_svm::founding_v5::{CLAIMS_FOUNDING_RECEIPT_BYTES_V5, ClaimsFoundingRequestV5};
use dclutch_core_contract::ContentId;
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{ProgramV3, ResolvedInvocationV3, ResolvedReceiptDependencyV3, RouteKindV3},
    v4::ProgramV4,
};
use dclutch_market_core_codec::{
    Identity, SERIES_CORE_ACK_BYTES_V1, SERIES_CORE_REQUEST_BYTES_V1, SERIES_CORE_REQUEST_MAGIC_V1,
    SERIES_FOUNDING_PERMIT_BYTES_V1, SERIES_OPEN_POST_RESOURCE_DIGEST_DOMAIN_V1, SeriesCoreAckV1,
    SeriesCoreActionV1, SeriesCoreFoundAckV2, SeriesCoreRequestV1,
};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_series_v3_kernel::ticket_content_id;
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;

use crate::{
    TradingSbfError,
    child_receipt_v3::{
        ExpectedReceiptProvenanceV4, ReceiptDeliveryV3, deliver_receipt_dependency_v3,
    },
    hot_v3::DowngradedEffectAccountsV3,
    projected_claims_composition_v4::AuthenticatedProjectedClaimsV4,
    projected_core_composition_v4::AuthenticatedProjectedCorePrefixV4,
    projected_custody_composition_v4::AuthenticatedProjectedCustodyPrefixV4,
    projected_market_v2::ProjectedMarketExecutionV2,
    series::{
        accounts::{SERIES_ROOT_ACCOUNT_BYTES_V3, commit_occurrence_after_ack},
        artifacts_v3::{
            SERIES_CONSUME_CORE_OPEN_ACCOUNT_COUNT_V3, SERIES_CORE_OPEN_RECEIPT_DEPENDENCIES_V3,
        },
        effect_v4::{
            SERIES_CONSUME_CLAIMS_ROUTE_V4, SERIES_CONSUME_OPEN_ROUTE_V4,
            series_consume_route_account_start_v4,
        },
        lifecycle::OccurrenceCommitPlanV3,
        state::SERIES_TICKET_STATE_BYTES_V3,
    },
};

const OPEN_INVOCATION_V4: u32 = 0;
const OPEN_ACCOUNT_COUNT_V4: usize = 37;
const CALLER: usize = 0;
const MARKET: usize = 1;
const PERMIT: usize = 2;
const RENT_CREDIT: usize = 3;
const RENT_PROGRAM: usize = 4;
const TRADING_PROGRAM: usize = 7;
const CLAIMS_PROGRAM: usize = 9;
const CUSTODY_PROGRAM: usize = 11;
const CORE_PROGRAM: usize = 13;
const ROOT: usize = 15;
const TICKET_STATE: usize = 16;
const TICKET_RAW: usize = 21;
const FUNDING_SOURCE: usize = 31;
const AGGREGATE: usize = 32;
const POSITION: usize = 33;
const ADMISSION: usize = 34;
const _: () = assert!(SERIES_CONSUME_CORE_OPEN_ACCOUNT_COUNT_V3 as usize == OPEN_ACCOUNT_COUNT_V4);
const _: () = assert!(SERIES_CORE_OPEN_RECEIPT_DEPENDENCIES_V3.len() == 1);

/// Exact terminal evidence retained by the combined outer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedProjectedOpenV4 {
    pub(crate) raw_request: [u8; SERIES_CORE_REQUEST_BYTES_V1],
    pub(crate) request_digest: [u8; 32],
    pub(crate) raw_acknowledgement: [u8; SERIES_CORE_ACK_BYTES_V1],
    pub(crate) producer: Pubkey,
    pub(crate) provenance: ExpectedReceiptProvenanceV4,
}

struct PreparedProjectedOpenV4 {
    invocation: ResolvedInvocationV3,
    request: SeriesCoreRequestV1,
    raw_request: [u8; SERIES_CORE_REQUEST_BYTES_V1],
    request_digest: [u8; 32],
    child_data: Vec<u8>,
    authority_seeds: CallerAuthoritySeedsV1,
    authority_bump: u8,
    candidate_root_tail: [u8; 64],
    candidate_ticket: [u8; 64],
    permit_lamports: u64,
    rent_credit_lamports: u64,
    root_lamports: u64,
    ticket_lamports: u64,
}

/// Execute Core Open, authenticate the return and terminal permit closure,
/// then commit the Trading-owned root and Ticket candidates last.
#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_projected_open_route_v4<'info>(
    program_id: &Pubkey,
    execution: ProjectedMarketExecutionV2<'_>,
    effect: ProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    effect_accounts: DowngradedEffectAccountsV3<'_, '_, 'info>,
    request_bank: &[u8],
    core_program: &AccountInfo<'info>,
    outer_root: &AccountInfo<'info>,
    outer_ticket: &AccountInfo<'info>,
    plan: OccurrenceCommitPlanV3,
    lock_prefix: &AuthenticatedProjectedCustodyPrefixV4,
    found_prefix: &AuthenticatedProjectedCorePrefixV4,
    claims_prefix: &AuthenticatedProjectedClaimsV4,
    provenance: ExpectedReceiptProvenanceV4,
) -> Result<AuthenticatedProjectedOpenV4, ProgramError> {
    let prepared = prepare(
        program_id,
        execution,
        effect,
        tail_count,
        scalars,
        identities,
        effect_accounts,
        request_bank,
        core_program,
        outer_root,
        outer_ticket,
        plan,
        lock_prefix,
        found_prefix,
        claims_prefix,
        provenance,
    )?;
    let mut child_accounts = invocation_accounts(prepared.invocation, effect_accounts)?;
    let metas = child_accounts
        .iter()
        .enumerate()
        .map(|(index, account)| {
            if account.is_writable {
                AccountMeta::new(*account.key, index == CALLER)
            } else {
                AccountMeta::new_readonly(*account.key, index == CALLER)
            }
        })
        .collect();
    let instruction = Instruction {
        program_id: *core_program.key,
        accounts: metas,
        data: prepared.child_data,
    };
    child_accounts.push(core_program.clone());
    let bump_seed = [prepared.authority_bump];
    let [domain, release, market, role, context, digest] = prepared.authority_seeds.as_slices();
    invoke_signed(
        &instruction,
        &child_accounts,
        &[&[domain, release, market, role, context, digest, &bump_seed]],
    )
    .map_err(|_| TradingSbfError::Transition)?;
    let (producer, returned) = get_return_data().ok_or(TradingSbfError::Transition)?;
    let raw_acknowledgement: [u8; SERIES_CORE_ACK_BYTES_V1] = returned
        .as_slice()
        .try_into()
        .map_err(|_| TradingSbfError::Transition)?;
    let acknowledgement = authenticate_open_result(
        prepared.request,
        prepared.request_digest,
        claims_prefix.raw_receipt(),
        prepared.candidate_root_tail,
        prepared.candidate_ticket,
        &child_accounts,
        producer,
        *core_program.key,
        &raw_acknowledgement,
        prepared.permit_lamports,
        prepared.rent_credit_lamports,
    )?;
    commit_occurrence_after_ack(
        outer_root,
        outer_ticket,
        plan,
        acknowledgement,
        identity(core_program.key.to_bytes())?,
        identity(prepared.request_digest)?,
        acknowledgement.post_resource_digest(),
    )?;
    authenticate_replay_commit(
        outer_root,
        outer_ticket,
        prepared.candidate_root_tail,
        prepared.candidate_ticket,
        prepared.root_lamports,
        prepared.ticket_lamports,
    )?;
    Ok(AuthenticatedProjectedOpenV4 {
        raw_request: prepared.raw_request,
        request_digest: prepared.request_digest,
        raw_acknowledgement,
        producer,
        provenance,
    })
}

#[allow(clippy::too_many_arguments)]
fn prepare(
    program_id: &Pubkey,
    execution: ProjectedMarketExecutionV2<'_>,
    effect: ProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    effect_accounts: DowngradedEffectAccountsV3<'_, '_, '_>,
    request_bank: &[u8],
    core_program: &AccountInfo<'_>,
    outer_root: &AccountInfo<'_>,
    outer_ticket: &AccountInfo<'_>,
    plan: OccurrenceCommitPlanV3,
    lock_prefix: &AuthenticatedProjectedCustodyPrefixV4,
    found_prefix: &AuthenticatedProjectedCorePrefixV4,
    claims_prefix: &AuthenticatedProjectedClaimsV4,
    provenance: ExpectedReceiptProvenanceV4,
) -> Result<PreparedProjectedOpenV4, ProgramError> {
    let funding_count = u16::from(found_prefix.found_span().funding_count());
    let base = effect.base();
    if !core_program.executable
        || core_program.is_signer
        || core_program.is_writable
        || !outer_root.is_writable
        || outer_root.is_signer
        || outer_root.executable
        || outer_root.owner != program_id
        || outer_root.data_len() != SERIES_ROOT_ACCOUNT_BYTES_V3
        || !outer_ticket.is_writable
        || outer_ticket.is_signer
        || outer_ticket.executable
        || outer_ticket.owner != program_id
        || outer_ticket.data_len() != SERIES_TICKET_STATE_BYTES_V3
        || effect
            .account_count(tail_count, scalars)
            .map_err(|_| TradingSbfError::Content)?
            != effect_accounts.len()
        || base
            .request_bytes(tail_count)
            .map_err(|_| TradingSbfError::Content)?
            != request_bank.len()
        || base
            .invocation_count(
                SERIES_CONSUME_OPEN_ROUTE_V4,
                tail_count,
                scalars,
                identities,
            )
            .map_err(|_| TradingSbfError::Content)?
            != 1
        || lock_prefix.route() != 0
        || lock_prefix.invocation() != 0
        || found_prefix.route() != 1
        || found_prefix.invocation() != 0
        || claims_prefix.route() != SERIES_CONSUME_CLAIMS_ROUTE_V4
        || claims_prefix.invocation() != 0
        || provenance.context_digest == [0; 32]
        || provenance.request_kind != SERIES_CORE_REQUEST_MAGIC_V1
        || provenance.request_digest == [0; 32]
    {
        return Err(TradingSbfError::Content.into());
    }
    let resolved = effect
        .resolved_invocation(
            SERIES_CONSUME_OPEN_ROUTE_V4,
            OPEN_INVOCATION_V4,
            tail_count,
            scalars,
            identities,
        )
        .map_err(|_| TradingSbfError::Content)?;
    validate_invocation(
        base,
        resolved.invocation,
        resolved.borrowed_range_count(),
        funding_count,
    )?;
    let end = resolved
        .invocation
        .request_offset
        .checked_add(resolved.invocation.request_len)
        .ok_or(TradingSbfError::Content)?;
    let request_bytes = request_bank
        .get(resolved.invocation.request_offset..end)
        .ok_or(TradingSbfError::Content)?;
    let raw_request: [u8; SERIES_CORE_REQUEST_BYTES_V1] = request_bytes
        .try_into()
        .map_err(|_| TradingSbfError::Content)?;
    let request =
        SeriesCoreRequestV1::decode(&raw_request).map_err(|_| TradingSbfError::Content)?;
    if request.action() != SeriesCoreActionV1::Consume
        || &raw_request != found_prefix.raw_request()
        || plan.core_request() != Some(request)
    {
        return Err(TradingSbfError::Content.into());
    }
    let borrowed = effect
        .resolved_borrowed_range(SERIES_CONSUME_OPEN_ROUTE_V4, 0, scalars)
        .map_err(|_| TradingSbfError::Content)?;
    let witness = borrowed
        .slice(execution.family_request())
        .map_err(|_| TradingSbfError::Content)?;
    if witness != execution.witness() {
        return Err(TradingSbfError::Content.into());
    }
    let claims = ClaimsFoundingRequestV5::decode(claims_prefix.raw_request())
        .map_err(|_| TradingSbfError::Content)?;
    authenticate_prefix_join(
        request,
        claims,
        claims_prefix,
        lock_prefix,
        found_prefix,
        *program_id,
        *core_program.key,
    )?;
    let child_accounts = invocation_accounts(resolved.invocation, effect_accounts)?;
    let request_digest = hash(request_bytes).to_bytes();
    let ticket_context = ticket_context_from_frame(&child_accounts)?;
    let authority_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(request.release_set().to_bytes()).map_err(|_| TradingSbfError::Content)?,
        request.market().ok_or(TradingSbfError::Content)?.to_bytes(),
        ExecutionRoleV1::Trading,
        ticket_context,
        request_digest,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let (authority, authority_bump) =
        Pubkey::find_program_address(&authority_seeds.as_slices(), program_id);
    authenticate_frame(
        program_id,
        core_program,
        outer_root,
        outer_ticket,
        request,
        claims,
        lock_prefix.producer(),
        claims_prefix.producer(),
        found_prefix,
        authority,
        &child_accounts,
    )?;
    let (candidate_root_tail, candidate_ticket) = plan
        .candidate_bytes()
        .map_err(|_| TradingSbfError::Content)?;
    let permit = child_accounts.get(PERMIT).ok_or(TradingSbfError::Content)?;
    let rent_credit = child_accounts
        .get(RENT_CREDIT)
        .ok_or(TradingSbfError::Content)?;
    if permit.owner != core_program.key || permit.data_len() != SERIES_FOUNDING_PERMIT_BYTES_V1 {
        return Err(TradingSbfError::Content.into());
    }
    let mut child_data = request_bytes.to_vec();
    child_data.extend_from_slice(witness);
    // `core-sbf` routes this to `series_open` only when the tail carries
    // CLAIMS_FOUNDING_RECEIPT_MAGIC_V5.
    deliver_receipt_dependency_v3(
        resolved.invocation,
        &mut child_data,
        Some(claims_prefix.raw_receipt()),
        ReceiptDeliveryV3::ExactSuffix,
    )?;
    Ok(PreparedProjectedOpenV4 {
        invocation: resolved.invocation,
        request,
        raw_request,
        request_digest,
        child_data,
        authority_seeds,
        authority_bump,
        candidate_root_tail,
        candidate_ticket,
        permit_lamports: permit.lamports(),
        rent_credit_lamports: rent_credit.lamports(),
        root_lamports: outer_root.lamports(),
        ticket_lamports: outer_ticket.lamports(),
    })
}

fn validate_invocation(
    effect: ProgramV3<'_>,
    invocation: ResolvedInvocationV3,
    borrowed_range_count: u16,
    funding_count: u16,
) -> Result<(), ProgramError> {
    let dependency = ResolvedReceiptDependencyV3 {
        producer_role: FixedRole::Claims,
        producer_route: SERIES_CONSUME_CLAIMS_ROUTE_V4,
        producer_invocation: 0,
        expected_receipt_bytes: u16::try_from(CLAIMS_FOUNDING_RECEIPT_BYTES_V5)
            .map_err(|_| TradingSbfError::Content)?,
    };
    if invocation.role != FixedRole::Core
        || invocation.kind != RouteKindV3::Once
        || invocation.item.is_some()
        || invocation.fixed_account_start
            != series_consume_route_account_start_v4(SERIES_CONSUME_OPEN_ROUTE_V4, funding_count)
                .ok_or(TradingSbfError::Content)?
        || usize::from(invocation.fixed_account_count) != OPEN_ACCOUNT_COUNT_V4
        || invocation.item_account_count != 0
        || invocation.repeated_item_count != 0
        || invocation.request_len != SERIES_CORE_REQUEST_BYTES_V1
        || borrowed_range_count != 1
        || usize::from(invocation.receipt_dependencies.len()) != 1
        || invocation.receipt_dependency != Some(dependency)
        || effect
            .resolved_receipt_dependency(invocation.receipt_dependencies, 0)
            .map_err(|_| TradingSbfError::Content)?
            != dependency
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

fn invocation_accounts<'info>(
    invocation: ResolvedInvocationV3,
    accounts: DowngradedEffectAccountsV3<'_, '_, 'info>,
) -> Result<Vec<AccountInfo<'info>>, ProgramError> {
    let start = usize::from(invocation.fixed_account_start);
    let end = start
        .checked_add(usize::from(invocation.fixed_account_count))
        .ok_or(TradingSbfError::Content)?;
    let mut output = Vec::new();
    accounts.reserve_invocation_frame(&mut output, invocation)?;
    accounts.extend_window(
        &mut output,
        start,
        end.checked_sub(start).ok_or(TradingSbfError::Content)?,
    )?;
    Ok(output)
}

fn ticket_context_from_frame(accounts: &[AccountInfo<'_>]) -> Result<[u8; 32], ProgramError> {
    let data = accounts
        .get(TICKET_RAW)
        .ok_or(TradingSbfError::Content)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    ticket_content_id(&data)
        .map(|content| content.to_bytes())
        .map_err(|_| TradingSbfError::Content.into())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_prefix_join(
    request: SeriesCoreRequestV1,
    claims: ClaimsFoundingRequestV5,
    claims_prefix: &AuthenticatedProjectedClaimsV4,
    lock_prefix: &AuthenticatedProjectedCustodyPrefixV4,
    found_prefix: &AuthenticatedProjectedCorePrefixV4,
    trading_program: Pubkey,
    core_program: Pubkey,
) -> Result<(), ProgramError> {
    let found_ack = SeriesCoreFoundAckV2::decode(found_prefix.raw_acknowledgement())
        .map_err(|_| TradingSbfError::Content)?;
    if claims_prefix.producer().to_bytes() != claims.claims_program()
        || found_prefix.producer() != core_program
        || request.release_set().to_bytes() != claims.release_set()
        || request
            .market()
            .is_none_or(|market| market.to_bytes() != claims.market())
        || request.market_generation() != Some(claims.generation())
        || request.hoard_principal() != claims.collateral_transferred()
        || claims.trading_program() != trading_program.to_bytes()
        || found_ack.permit().to_bytes() == [0; 32]
        || found_ack.market().to_bytes() != claims.market()
        || found_ack.release_set().to_bytes() != claims.release_set()
        || lock_prefix.producer().to_bytes() == [0; 32]
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_frame(
    program_id: &Pubkey,
    core_program: &AccountInfo<'_>,
    outer_root: &AccountInfo<'_>,
    outer_ticket: &AccountInfo<'_>,
    request: SeriesCoreRequestV1,
    claims: ClaimsFoundingRequestV5,
    custody_program: Pubkey,
    claims_program: Pubkey,
    found_prefix: &AuthenticatedProjectedCorePrefixV4,
    authority: Pubkey,
    accounts: &[AccountInfo<'_>],
) -> Result<(), ProgramError> {
    let ack = SeriesCoreFoundAckV2::decode(found_prefix.raw_acknowledgement())
        .map_err(|_| TradingSbfError::Content)?;
    let expected = [
        (CALLER, authority),
        (
            MARKET,
            Pubkey::new_from_array(request.market().ok_or(TradingSbfError::Content)?.to_bytes()),
        ),
        (PERMIT, Pubkey::new_from_array(ack.permit().to_bytes())),
        (RENT_CREDIT, Pubkey::new_from_array(claims.rent_credit())),
        (RENT_PROGRAM, Pubkey::new_from_array(claims.rent_program())),
        (TRADING_PROGRAM, *program_id),
        (CLAIMS_PROGRAM, claims_program),
        (CUSTODY_PROGRAM, custody_program),
        (CORE_PROGRAM, *core_program.key),
        (ROOT, *outer_root.key),
        (TICKET_STATE, *outer_ticket.key),
        (
            FUNDING_SOURCE,
            Pubkey::new_from_array(claims.funding_source()),
        ),
        (AGGREGATE, Pubkey::new_from_array(claims.aggregate())),
        (POSITION, Pubkey::new_from_array(claims.position())),
        (ADMISSION, Pubkey::new_from_array(claims.admission())),
    ];
    if accounts.len() != OPEN_ACCOUNT_COUNT_V4
        || expected.iter().any(|(index, key)| {
            accounts
                .get(*index)
                .is_none_or(|account| account.key != key)
        })
    {
        return Err(TradingSbfError::Release.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_open_result(
    request: SeriesCoreRequestV1,
    request_digest: [u8; 32],
    raw_claims: &[u8; CLAIMS_FOUNDING_RECEIPT_BYTES_V5],
    candidate_root: [u8; 64],
    candidate_ticket: [u8; 64],
    accounts: &[AccountInfo<'_>],
    producer: Pubkey,
    core_program: Pubkey,
    raw_ack: &[u8; SERIES_CORE_ACK_BYTES_V1],
    permit_lamports: u64,
    rent_credit_lamports: u64,
) -> Result<SeriesCoreAckV1, ProgramError> {
    let market = accounts.get(MARKET).ok_or(TradingSbfError::Transition)?;
    let permit = accounts.get(PERMIT).ok_or(TradingSbfError::Transition)?;
    let rent_credit = accounts
        .get(RENT_CREDIT)
        .ok_or(TradingSbfError::Transition)?;
    let market_data = market
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let post = hashv(&[
        SERIES_OPEN_POST_RESOURCE_DIGEST_DOMAIN_V1,
        &market_data,
        raw_claims,
        &candidate_root,
        &candidate_ticket,
    ])
    .to_bytes();
    drop(market_data);
    let acknowledgement =
        SeriesCoreAckV1::decode(raw_ack).map_err(|_| TradingSbfError::Transition)?;
    acknowledgement
        .validate_for(
            request,
            identity(core_program.to_bytes())?,
            identity(request_digest)?,
            identity(post)?,
        )
        .map_err(|_| TradingSbfError::Transition)?;
    let expected_rent_credit = rent_credit_lamports
        .checked_add(permit_lamports)
        .ok_or(TradingSbfError::Transition)?;
    if producer != core_program
        || permit.owner != &system_program::ID
        || permit.lamports() != 0
        || !permit.data_is_empty()
        || rent_credit.lamports() != expected_rent_credit
    {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(acknowledgement)
}

fn authenticate_replay_commit(
    root: &AccountInfo<'_>,
    ticket: &AccountInfo<'_>,
    expected_root: [u8; 64],
    expected_ticket: [u8; 64],
    root_lamports: u64,
    ticket_lamports: u64,
) -> Result<(), ProgramError> {
    let root_data = root
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let root_tail = root_data
        .get(root_data.len().saturating_sub(expected_root.len())..)
        .ok_or(TradingSbfError::Transition)?;
    let ticket_data = ticket
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    if root_tail != expected_root
        || ticket_data.as_ref() != expected_ticket
        || root.lamports() != root_lamports
        || ticket.lamports() != ticket_lamports
    {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(())
}

fn identity(bytes: [u8; 32]) -> Result<Identity, ProgramError> {
    Identity::new(bytes).map_err(|_| TradingSbfError::Transition.into())
}

#[cfg(test)]
mod tests {
    use alloc::{boxed::Box, vec};

    use super::*;

    fn id(byte: u8) -> Identity {
        Identity::new([byte; 32]).expect("identity")
    }

    fn request() -> SeriesCoreRequestV1 {
        SeriesCoreRequestV1::occurrence(
            SeriesCoreActionV1::Consume,
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            id(6),
            id(7),
            id(8),
            9,
            10,
            11,
            12,
            13,
            14,
            15,
        )
        .expect("request")
    }

    fn account(key: Pubkey, owner: Pubkey, lamports: u64, data: Vec<u8>) -> AccountInfo<'static> {
        AccountInfo::new(
            Box::leak(Box::new(key)),
            false,
            true,
            Box::leak(Box::new(lamports)),
            Box::leak(data.into_boxed_slice()),
            Box::leak(Box::new(owner)),
            false,
        )
    }

    struct OpenFixture {
        request: SeriesCoreRequestV1,
        core: Pubkey,
        accounts: Vec<AccountInfo<'static>>,
        claims: [u8; CLAIMS_FOUNDING_RECEIPT_BYTES_V5],
        root: [u8; 64],
        ticket: [u8; 64],
        acknowledgement: [u8; SERIES_CORE_ACK_BYTES_V1],
    }

    fn fixture() -> OpenFixture {
        let request = request();
        let core = Pubkey::new_from_array([20; 32]);
        let mut accounts = (0..OPEN_ACCOUNT_COUNT_V4 + 1)
            .map(|_| account(Pubkey::new_unique(), Pubkey::new_unique(), 1, Vec::new()))
            .collect::<Vec<_>>();
        *accounts.get_mut(MARKET).expect("market account") = account(
            Pubkey::new_from_array(request.market().expect("market").to_bytes()),
            core,
            2,
            vec![21; 64],
        );
        *accounts.get_mut(PERMIT).expect("permit account") =
            account(Pubkey::new_unique(), system_program::ID, 0, Vec::new());
        *accounts.get_mut(RENT_CREDIT).expect("rent account") =
            account(Pubkey::new_unique(), Pubkey::new_unique(), 8, Vec::new());
        let claims = [22; CLAIMS_FOUNDING_RECEIPT_BYTES_V5];
        let root = [23; 64];
        let ticket = [24; 64];
        let request_digest = hash(&request.encode().expect("request bytes")).to_bytes();
        let market_data = accounts
            .get(MARKET)
            .expect("market account")
            .try_borrow_data()
            .expect("market data");
        let post = hashv(&[
            SERIES_OPEN_POST_RESOURCE_DIGEST_DOMAIN_V1,
            &market_data,
            &claims,
            &root,
            &ticket,
        ])
        .to_bytes();
        drop(market_data);
        let acknowledgement = SeriesCoreAckV1::new(
            request,
            id(20),
            Identity::new(request_digest).expect("request digest"),
            Identity::new(post).expect("post resource"),
        )
        .encode()
        .expect("acknowledgement");
        OpenFixture {
            request,
            core,
            accounts,
            claims,
            root,
            ticket,
            acknowledgement,
        }
    }

    fn authenticate(value: &OpenFixture) -> Result<SeriesCoreAckV1, ProgramError> {
        let digest = hash(&value.request.encode().expect("request bytes")).to_bytes();
        authenticate_open_result(
            value.request,
            digest,
            &value.claims,
            value.root,
            value.ticket,
            &value.accounts,
            value.core,
            value.core,
            &value.acknowledgement,
            0,
            8,
        )
    }

    #[test]
    fn exact_open_ack_binds_market_claims_and_both_replay_candidates() {
        let exact = fixture();
        assert!(authenticate(&exact).is_ok());

        let mut wrong_root = fixture();
        wrong_root.root[0] ^= 1;
        assert_eq!(
            authenticate(&wrong_root),
            Err(TradingSbfError::Transition.into())
        );

        let mut wrong_ticket = fixture();
        wrong_ticket.ticket[0] ^= 1;
        assert_eq!(
            authenticate(&wrong_ticket),
            Err(TradingSbfError::Transition.into())
        );

        let mut wrong_claims = fixture();
        wrong_claims.claims[0] ^= 1;
        assert_eq!(
            authenticate(&wrong_claims),
            Err(TradingSbfError::Transition.into())
        );
    }

    #[test]
    fn producer_market_and_permit_rent_substitution_refuse() {
        let wrong_producer = fixture();
        let digest = hash(&wrong_producer.request.encode().expect("request bytes")).to_bytes();
        assert_eq!(
            authenticate_open_result(
                wrong_producer.request,
                digest,
                &wrong_producer.claims,
                wrong_producer.root,
                wrong_producer.ticket,
                &wrong_producer.accounts,
                Pubkey::new_unique(),
                wrong_producer.core,
                &wrong_producer.acknowledgement,
                0,
                8,
            ),
            Err(TradingSbfError::Transition.into())
        );

        let wrong_market = fixture();
        *wrong_market
            .accounts
            .get(MARKET)
            .expect("market")
            .try_borrow_mut_data()
            .expect("market data")
            .first_mut()
            .expect("market byte") ^= 1;
        assert_eq!(
            authenticate(&wrong_market),
            Err(TradingSbfError::Transition.into())
        );

        let wrong_rent = fixture();
        assert_eq!(
            authenticate_open_result(
                wrong_rent.request,
                hash(&wrong_rent.request.encode().expect("request bytes")).to_bytes(),
                &wrong_rent.claims,
                wrong_rent.root,
                wrong_rent.ticket,
                &wrong_rent.accounts,
                wrong_rent.core,
                wrong_rent.core,
                &wrong_rent.acknowledgement,
                1,
                8,
            ),
            Err(TradingSbfError::Transition.into())
        );
    }

    #[test]
    fn route_four_geometry_is_exact() {
        assert_eq!(SERIES_CONSUME_OPEN_ROUTE_V4, 4);
        assert_eq!(OPEN_ACCOUNT_COUNT_V4, 37);
        assert_eq!(SERIES_CORE_OPEN_RECEIPT_DEPENDENCIES_V3.len(), 1);
        assert_eq!(SERIES_CORE_ACK_BYTES_V1, 264);
    }
}
