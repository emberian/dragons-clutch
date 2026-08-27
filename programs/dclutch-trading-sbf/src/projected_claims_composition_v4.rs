//! Exact Claims-Founding continuation for an admitted projected-Market plan.

extern crate alloc;

use alloc::vec::Vec;

use dclutch_claims_svm::founding_v5::{
    CLAIMS_FOUNDING_POST_RESOURCE_DIGEST_DOMAIN_V5, CLAIMS_FOUNDING_RECEIPT_BYTES_V5,
    CLAIMS_FOUNDING_RECEIPT_MAGIC_V5, CLAIMS_FOUNDING_REQUEST_BYTES_V5,
    CLAIMS_FOUNDING_REQUEST_MAGIC_V5, ClaimsFoundingReceiptV5, ClaimsFoundingRequestV5,
};
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1, PROJECTED_CUSTODY_RECEIPT_BYTES_V1,
    ProjectedCustodyLockReceiptV1, ProjectedCustodyReceiptV1,
};
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{ProgramV3, ResolvedInvocationV3, ResolvedReceiptDependencyV3, RouteKindV3},
    v4::ProgramV4,
};
use dclutch_market_core_codec::{SERIES_CORE_FOUND_ACK_BYTES_V2, SeriesCoreFoundAckV2};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::{
    TradingSbfError,
    child_receipt_v3::{
        ChildReceiptBankV3, ExpectedReceiptProvenanceV4, ReceiptDeliveryV3,
        deliver_receipt_dependency_v3,
    },
    hot_v3::DowngradedEffectAccountsV3,
    projected_core_composition_v4::AuthenticatedProjectedCorePrefixV4,
    projected_custody_composition_v4::AuthenticatedProjectedCustodyPrefixV4,
    projected_realize_composition_v4::AuthenticatedProjectedRealizeV4,
    series::{
        artifacts_v3::{
            SERIES_CLAIMS_RECEIPT_DEPENDENCIES_V3, SERIES_CONSUME_CLAIMS_ACCOUNT_COUNT_V3,
        },
        effect_v4::{
            SERIES_CONSUME_CLAIMS_ROUTE_V4, SERIES_CONSUME_LOCK_ROUTE_V4,
            SERIES_CONSUME_REALIZE_ROUTE_V4, series_consume_route_account_start_v4,
        },
    },
};

const CLAIMS_INVOCATION_V4: u32 = 0;
const CLAIMS_ACCOUNT_COUNT_V4: usize = 32;
const AUTHORITY: usize = 0;
const PERMIT: usize = 1;
const AGGREGATE: usize = 2;
const POSITION: usize = 3;
const ADMISSION: usize = 4;
const FUNDING_SOURCE: usize = 5;
const HOARD: usize = 6;
const CUSTODY_REPLAY: usize = 7;
const MARKET: usize = 18;
const CLAIMS_PROGRAM: usize = 21;
const CORE_PROGRAM: usize = 23;
const TRADING_PROGRAM: usize = 25;
const CUSTODY_PROGRAM: usize = 27;
const FOUNDER: usize = 29;
const RENT_CREDIT: usize = 30;
const RENT_PROGRAM: usize = 31;
const _: () = assert!(SERIES_CONSUME_CLAIMS_ACCOUNT_COUNT_V3 as usize == CLAIMS_ACCOUNT_COUNT_V4);
const _: () = assert!(SERIES_CLAIMS_RECEIPT_DEPENDENCIES_V3.len() == 2);

/// Exact executed Claims-Founding fact retained for final Core Open.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedProjectedClaimsV4 {
    route: u16,
    invocation: u32,
    raw_request: [u8; CLAIMS_FOUNDING_REQUEST_BYTES_V5],
    request_digest: [u8; 32],
    raw_receipt: [u8; CLAIMS_FOUNDING_RECEIPT_BYTES_V5],
    producer: Pubkey,
    provenance: ExpectedReceiptProvenanceV4,
}

impl AuthenticatedProjectedClaimsV4 {
    pub(crate) const fn route(&self) -> u16 {
        self.route
    }

    pub(crate) const fn invocation(&self) -> u32 {
        self.invocation
    }

    pub(crate) const fn raw_request(&self) -> &[u8; CLAIMS_FOUNDING_REQUEST_BYTES_V5] {
        &self.raw_request
    }

    pub(crate) const fn request_digest(&self) -> [u8; 32] {
        self.request_digest
    }

    pub(crate) const fn raw_receipt(&self) -> &[u8; CLAIMS_FOUNDING_RECEIPT_BYTES_V5] {
        &self.raw_receipt
    }

    pub(crate) const fn producer(&self) -> Pubkey {
        self.producer
    }

    pub(crate) const fn provenance(&self) -> ExpectedReceiptProvenanceV4 {
        self.provenance
    }

    /// Seed the exact route-three result into the ephemeral receipt bank.
    pub(crate) fn record_into(self, bank: &mut ChildReceiptBankV3) -> Result<(), ProgramError> {
        bank.record_exact(
            FixedRole::Claims,
            self.route,
            self.invocation,
            self.producer,
            self.provenance.context_digest,
            self.provenance.request_kind,
            self.provenance.request_digest,
            CLAIMS_FOUNDING_RECEIPT_MAGIC_V5,
            self.raw_receipt.to_vec(),
        )
    }
}

struct PreparedProjectedClaimsV4 {
    invocation: ResolvedInvocationV3,
    request: ClaimsFoundingRequestV5,
    raw_request: [u8; CLAIMS_FOUNDING_REQUEST_BYTES_V5],
    request_digest: [u8; 32],
    child_data: Vec<u8>,
    authority_seeds: CallerAuthoritySeedsV1,
    authority_bump: u8,
}

struct ClaimsPostResourceDigestsV4 {
    aggregate: [u8; 32],
    position: [u8; 32],
    admission: [u8; 32],
    combined: [u8; 32],
}

/// Execute global route three with the exact retained Lock and Realize receipts.
#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_projected_claims_route_v4<'info>(
    program_id: &Pubkey,
    effect: ProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    effect_accounts: DowngradedEffectAccountsV3<'_, '_, 'info>,
    request_bank: &[u8],
    claims_program: &AccountInfo<'info>,
    lock_prefix: &AuthenticatedProjectedCustodyPrefixV4,
    core_prefix: &AuthenticatedProjectedCorePrefixV4,
    realize_prefix: &AuthenticatedProjectedRealizeV4,
    provenance: ExpectedReceiptProvenanceV4,
) -> Result<AuthenticatedProjectedClaimsV4, ProgramError> {
    let prepared = prepare(
        program_id,
        effect,
        tail_count,
        scalars,
        identities,
        effect_accounts,
        request_bank,
        claims_program,
        lock_prefix,
        core_prefix,
        realize_prefix,
        provenance,
    )?;
    let mut child_accounts = invocation_accounts(prepared.invocation, effect_accounts)?;
    let mut metas = Vec::with_capacity(child_accounts.len());
    for (index, account) in child_accounts.iter().enumerate() {
        let signer = index == AUTHORITY;
        metas.push(if account.is_writable {
            AccountMeta::new(*account.key, signer)
        } else {
            AccountMeta::new_readonly(*account.key, signer)
        });
    }
    let instruction = Instruction {
        program_id: *claims_program.key,
        accounts: metas,
        data: prepared.child_data,
    };
    child_accounts.push(claims_program.clone());
    let bump_seed = [prepared.authority_bump];
    let [domain, release, market, role, context, digest] = prepared.authority_seeds.as_slices();
    invoke_signed(
        &instruction,
        &child_accounts,
        &[&[domain, release, market, role, context, digest, &bump_seed]],
    )
    .map_err(|_| TradingSbfError::Transition)?;
    let (producer, return_bytes) = get_return_data().ok_or(TradingSbfError::Transition)?;
    let raw_receipt: [u8; CLAIMS_FOUNDING_RECEIPT_BYTES_V5] = return_bytes
        .as_slice()
        .try_into()
        .map_err(|_| TradingSbfError::Transition)?;
    authenticate_result(
        prepared.request,
        prepared.request_digest,
        &child_accounts,
        producer,
        *claims_program.key,
        &raw_receipt,
    )?;
    Ok(AuthenticatedProjectedClaimsV4 {
        route: SERIES_CONSUME_CLAIMS_ROUTE_V4,
        invocation: CLAIMS_INVOCATION_V4,
        raw_request: prepared.raw_request,
        request_digest: prepared.request_digest,
        raw_receipt,
        producer,
        provenance,
    })
}

#[allow(clippy::too_many_arguments)]
fn prepare(
    program_id: &Pubkey,
    effect: ProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    effect_accounts: DowngradedEffectAccountsV3<'_, '_, '_>,
    request_bank: &[u8],
    claims_program: &AccountInfo<'_>,
    lock_prefix: &AuthenticatedProjectedCustodyPrefixV4,
    core_prefix: &AuthenticatedProjectedCorePrefixV4,
    realize_prefix: &AuthenticatedProjectedRealizeV4,
    provenance: ExpectedReceiptProvenanceV4,
) -> Result<PreparedProjectedClaimsV4, ProgramError> {
    let funding_count = u16::from(core_prefix.found_span().funding_count());
    let base = effect.base();
    if !claims_program.executable
        || claims_program.is_signer
        || claims_program.is_writable
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
                SERIES_CONSUME_CLAIMS_ROUTE_V4,
                tail_count,
                scalars,
                identities,
            )
            .map_err(|_| TradingSbfError::Content)?
            != 1
        || lock_prefix.route() != SERIES_CONSUME_LOCK_ROUTE_V4
        || lock_prefix.invocation() != 0
        || core_prefix.route() != 1
        || core_prefix.invocation() != 0
        || realize_prefix.route() != SERIES_CONSUME_REALIZE_ROUTE_V4
        || realize_prefix.invocation() != 0
        || provenance.context_digest == [0; 32]
        || provenance.request_kind != CLAIMS_FOUNDING_REQUEST_MAGIC_V5
        || provenance.request_digest == [0; 32]
    {
        return Err(TradingSbfError::Content.into());
    }
    let resolved = effect
        .resolved_invocation(
            SERIES_CONSUME_CLAIMS_ROUTE_V4,
            CLAIMS_INVOCATION_V4,
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
    let request_end = resolved
        .invocation
        .request_offset
        .checked_add(resolved.invocation.request_len)
        .ok_or(TradingSbfError::Content)?;
    let request_bytes = request_bank
        .get(resolved.invocation.request_offset..request_end)
        .ok_or(TradingSbfError::Content)?;
    let raw_request: [u8; CLAIMS_FOUNDING_REQUEST_BYTES_V5] = request_bytes
        .try_into()
        .map_err(|_| TradingSbfError::Content)?;
    let request =
        ClaimsFoundingRequestV5::decode(&raw_request).map_err(|_| TradingSbfError::Content)?;
    let request_digest = hash(request_bytes).to_bytes();
    authenticate_prefix_join(
        request,
        lock_prefix.request_digest(),
        lock_prefix.raw_receipt(),
        lock_prefix.producer(),
        realize_prefix.raw_receipt(),
        realize_prefix.producer(),
        core_prefix,
        *program_id,
        *claims_program.key,
    )?;
    let child_accounts = invocation_accounts(resolved.invocation, effect_accounts)?;
    authenticate_frame(
        program_id,
        claims_program,
        request,
        request_digest,
        core_prefix,
        lock_prefix.producer(),
        &child_accounts,
    )?;
    let mut dependencies = Vec::with_capacity(
        PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1 + PROJECTED_CUSTODY_RECEIPT_BYTES_V1,
    );
    dependencies.extend_from_slice(lock_prefix.raw_receipt());
    dependencies.extend_from_slice(realize_prefix.raw_receipt());
    let mut child_data = request_bytes.to_vec();
    // `claims-sbf::founding_v5::decode_instruction` requires exactly the
    // request followed by the lock receipt and the projected receipt.
    deliver_receipt_dependency_v3(
        resolved.invocation,
        &mut child_data,
        Some(&dependencies),
        ReceiptDeliveryV3::ExactSuffix,
    )?;
    let authority_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(request.release_set()).map_err(|_| TradingSbfError::Content)?,
        request.market(),
        ExecutionRoleV1::Trading,
        request.founding_intent_digest(),
        request_digest,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let (_, authority_bump) =
        Pubkey::find_program_address(&authority_seeds.as_slices(), program_id);
    Ok(PreparedProjectedClaimsV4 {
        invocation: resolved.invocation,
        request,
        raw_request,
        request_digest,
        child_data,
        authority_seeds,
        authority_bump,
    })
}

fn validate_invocation(
    effect: ProgramV3<'_>,
    invocation: ResolvedInvocationV3,
    borrowed_range_count: u16,
    funding_count: u16,
) -> Result<(), ProgramError> {
    let expected_start =
        series_consume_route_account_start_v4(SERIES_CONSUME_CLAIMS_ROUTE_V4, funding_count)
            .ok_or(TradingSbfError::Content)?;
    let lock_dependency = ResolvedReceiptDependencyV3 {
        producer_role: FixedRole::Custody,
        producer_route: SERIES_CONSUME_LOCK_ROUTE_V4,
        producer_invocation: 0,
        expected_receipt_bytes: u16::try_from(PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1)
            .map_err(|_| TradingSbfError::Content)?,
    };
    let realize_dependency = ResolvedReceiptDependencyV3 {
        producer_role: FixedRole::Custody,
        producer_route: SERIES_CONSUME_REALIZE_ROUTE_V4,
        producer_invocation: 0,
        expected_receipt_bytes: u16::try_from(PROJECTED_CUSTODY_RECEIPT_BYTES_V1)
            .map_err(|_| TradingSbfError::Content)?,
    };
    if invocation.role != FixedRole::Claims
        || invocation.kind != RouteKindV3::Once
        || invocation.item.is_some()
        || invocation.fixed_account_start != expected_start
        || usize::from(invocation.fixed_account_count) != CLAIMS_ACCOUNT_COUNT_V4
        || invocation.item_account_count != 0
        || invocation.repeated_item_count != 0
        || invocation.request_len != CLAIMS_FOUNDING_REQUEST_BYTES_V5
        || borrowed_range_count != 0
        || usize::from(invocation.receipt_dependencies.len())
            != SERIES_CLAIMS_RECEIPT_DEPENDENCIES_V3.len()
        || invocation.receipt_dependency != Some(lock_dependency)
        || effect
            .resolved_receipt_dependency(invocation.receipt_dependencies, 0)
            .map_err(|_| TradingSbfError::Content)?
            != lock_dependency
        || effect
            .resolved_receipt_dependency(invocation.receipt_dependencies, 1)
            .map_err(|_| TradingSbfError::Content)?
            != realize_dependency
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
    let mut output = accounts.invocation_frame(invocation)?;
    accounts.extend_window(
        &mut output,
        start,
        end.checked_sub(start).ok_or(TradingSbfError::Content)?,
    )?;
    Ok(output)
}

#[allow(clippy::too_many_arguments)]
fn authenticate_prefix_join(
    request: ClaimsFoundingRequestV5,
    lock_request_digest: [u8; 32],
    raw_lock: &[u8; PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1],
    lock_producer: Pubkey,
    raw_realize: &[u8; PROJECTED_CUSTODY_RECEIPT_BYTES_V1],
    realize_producer: Pubkey,
    core_prefix: &AuthenticatedProjectedCorePrefixV4,
    trading_program: Pubkey,
    claims_program: Pubkey,
) -> Result<(), ProgramError> {
    authenticate_receipt_join(
        request,
        lock_request_digest,
        raw_lock,
        lock_producer,
        raw_realize,
        realize_producer,
        trading_program,
        claims_program,
    )?;
    let core_ack = SeriesCoreFoundAckV2::decode(core_prefix.raw_acknowledgement())
        .map_err(|_| TradingSbfError::Content)?;
    if core_prefix.raw_acknowledgement().len() != SERIES_CORE_FOUND_ACK_BYTES_V2
        || request.generation() != core_ack.market_generation()
        || core_ack.permit().to_bytes() == [0; 32]
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_receipt_join(
    request: ClaimsFoundingRequestV5,
    lock_request_digest: [u8; 32],
    raw_lock: &[u8; PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1],
    lock_producer: Pubkey,
    raw_realize: &[u8; PROJECTED_CUSTODY_RECEIPT_BYTES_V1],
    realize_producer: Pubkey,
    trading_program: Pubkey,
    claims_program: Pubkey,
) -> Result<(), ProgramError> {
    let lock =
        ProjectedCustodyLockReceiptV1::decode(raw_lock).map_err(|_| TradingSbfError::Content)?;
    let realize =
        ProjectedCustodyReceiptV1::decode(raw_realize).map_err(|_| TradingSbfError::Content)?;
    if lock_producer != realize_producer
        || request.release_set() != lock.release_set
        || request.release_set() != realize.release_set
        || request.market() != lock.market
        || request.market() != realize.market
        || request.funding_source() != lock.source_vault
        || request.hoard() != lock.hoard_vault
        || request.hoard() != realize.hoard_vault
        || request.rent_credit() != lock.rent_credit
        || request.rent_credit() != realize.rent_credit
        || request.custody_request_digest() != lock_request_digest
        || request.custody_request_digest() != lock.request_digest
        || request.custody_receipt_digest() != hash(raw_lock).to_bytes()
        || request.collateral_transferred() != lock.amount
        || request.collateral_transferred() != realize.amount
        || !realize.realized
        || realize.aborted_open
        || lock.context_digest != realize.context_digest
        || lock.resulting_revision.checked_add(1) != Some(realize.resulting_revision)
        || request.trading_program() != trading_program.to_bytes()
        || request.claims_program() != claims_program.to_bytes()
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_frame(
    program_id: &Pubkey,
    claims_program: &AccountInfo<'_>,
    request: ClaimsFoundingRequestV5,
    request_digest: [u8; 32],
    core_prefix: &AuthenticatedProjectedCorePrefixV4,
    custody_program: Pubkey,
    accounts: &[AccountInfo<'_>],
) -> Result<(), ProgramError> {
    let seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(request.release_set()).map_err(|_| TradingSbfError::Content)?,
        request.market(),
        ExecutionRoleV1::Trading,
        request.founding_intent_digest(),
        request_digest,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let authority = Pubkey::find_program_address(&seeds.as_slices(), program_id).0;
    let ack = SeriesCoreFoundAckV2::decode(core_prefix.raw_acknowledgement())
        .map_err(|_| TradingSbfError::Content)?;
    let expected = [
        (AUTHORITY, authority),
        (PERMIT, Pubkey::new_from_array(ack.permit().to_bytes())),
        (AGGREGATE, Pubkey::new_from_array(request.aggregate())),
        (POSITION, Pubkey::new_from_array(request.position())),
        (ADMISSION, Pubkey::new_from_array(request.admission())),
        (
            FUNDING_SOURCE,
            Pubkey::new_from_array(request.funding_source()),
        ),
        (HOARD, Pubkey::new_from_array(request.hoard())),
        (
            CUSTODY_REPLAY,
            Pubkey::new_from_array(request.custody_replay()),
        ),
        (MARKET, Pubkey::new_from_array(request.market())),
        (CLAIMS_PROGRAM, *claims_program.key),
        (CORE_PROGRAM, core_prefix.producer()),
        (TRADING_PROGRAM, *program_id),
        (CUSTODY_PROGRAM, custody_program),
        (FOUNDER, Pubkey::new_from_array(request.founder())),
        (RENT_CREDIT, Pubkey::new_from_array(request.rent_credit())),
        (RENT_PROGRAM, Pubkey::new_from_array(request.rent_program())),
    ];
    if accounts.len() != CLAIMS_ACCOUNT_COUNT_V4
        || accounts
            .iter()
            .any(|account| account.key == claims_program.key && !account.executable)
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

fn authenticate_result(
    request: ClaimsFoundingRequestV5,
    request_digest: [u8; 32],
    accounts: &[AccountInfo<'_>],
    producer: Pubkey,
    claims_program: Pubkey,
    raw_receipt: &[u8; CLAIMS_FOUNDING_RECEIPT_BYTES_V5],
) -> Result<(), ProgramError> {
    let receipt =
        ClaimsFoundingReceiptV5::decode(raw_receipt).map_err(|_| TradingSbfError::Transition)?;
    receipt
        .verify_for(&request, request_digest)
        .map_err(|_| TradingSbfError::Transition)?;
    let post = post_resource_digests(accounts)?;
    if producer != claims_program
        || request.claims_program() != claims_program.to_bytes()
        || receipt.to_bytes() != *raw_receipt
        || receipt.aggregate_digest() != post.aggregate
        || receipt.position_digest() != post.position
        || receipt.admission_digest() != post.admission
        || receipt.post_resource_digest() != post.combined
    {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(())
}

fn post_resource_digests(
    accounts: &[AccountInfo<'_>],
) -> Result<ClaimsPostResourceDigestsV4, ProgramError> {
    let aggregate = accounts
        .get(AGGREGATE)
        .ok_or(TradingSbfError::Content)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let position = accounts
        .get(POSITION)
        .ok_or(TradingSbfError::Content)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let admission = accounts
        .get(ADMISSION)
        .ok_or(TradingSbfError::Content)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    Ok(ClaimsPostResourceDigestsV4 {
        aggregate: hash(&aggregate).to_bytes(),
        position: hash(&position).to_bytes(),
        admission: hash(&admission).to_bytes(),
        combined: hashv(&[
            CLAIMS_FOUNDING_POST_RESOURCE_DIGEST_DOMAIN_V5,
            &aggregate,
            &position,
            &admission,
        ])
        .to_bytes(),
    })
}

#[cfg(test)]
mod tests {
    use alloc::{boxed::Box, vec};

    use dclutch_claims_svm::founding_v5::ClaimsFoundingRequestInputV5;

    use super::*;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn request(
        claims: Pubkey,
        trading: Pubkey,
        lock: ProjectedCustodyLockReceiptV1,
        lock_bytes: &[u8; PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1],
    ) -> ClaimsFoundingRequestV5 {
        ClaimsFoundingRequestV5::new(ClaimsFoundingRequestInputV5 {
            release_set: lock.release_set,
            market: lock.market,
            product_record_digest: id(3),
            product_instance_id: id(4),
            linked_basis_record_digest: id(5),
            semantic_basis_id: id(6),
            founder: id(7),
            founding_intent_digest: id(8),
            aggregate: id(9),
            position: id(10),
            admission: id(11),
            funding_source: lock.source_vault,
            hoard: lock.hoard_vault,
            custody_replay: id(14),
            rent_credit: lock.rent_credit,
            rent_program: id(16),
            claims_program: claims.to_bytes(),
            trading_program: trading.to_bytes(),
            custody_request_digest: lock.request_digest,
            custody_receipt_digest: hash(lock_bytes).to_bytes(),
            generation: 21,
            claim_count: 5,
            quantity: 7,
            basis_scale: 11,
            pre_source_amount: 77,
            post_source_amount: 0,
            pre_hoard_amount: 23,
            post_hoard_amount: 100,
            pre_custody_revision: 0,
            post_custody_revision: 1,
            aggregate_rent_principal: 30,
            position_rent_principal: 31,
            admission_rent_principal: 32,
            observed_aggregate_lamports: 33,
            observed_position_lamports: 34,
            observed_admission_lamports: 35,
            pre_aggregate_revision: 0,
            post_aggregate_revision: 1,
            pre_position_revision: 0,
            post_position_revision: 1,
        })
        .expect("request")
    }

    fn lock_receipt() -> ProjectedCustodyLockReceiptV1 {
        ProjectedCustodyLockReceiptV1 {
            market: id(41),
            release_set: id(42),
            context_digest: id(43),
            source_vault: id(44),
            source_replay: id(45),
            hoard_vault: id(46),
            rent_credit: id(47),
            request_digest: id(48),
            amount: 77,
            source_vault_rent_lamports: 9,
            source_replay_rent_lamports: 10,
            resulting_revision: 3,
        }
    }

    fn realized_receipt(lock: ProjectedCustodyLockReceiptV1) -> ProjectedCustodyReceiptV1 {
        ProjectedCustodyReceiptV1 {
            realized: true,
            aborted_open: false,
            market: lock.market,
            release_set: lock.release_set,
            parent_capability_root: id(49),
            context_digest: lock.context_digest,
            hoard_vault: lock.hoard_vault,
            amount: lock.amount,
            request_digest: id(50),
            market_state_digest: id(51),
            rent_credit: lock.rent_credit,
            resulting_revision: 4,
        }
    }

    fn account(data: Vec<u8>) -> AccountInfo<'static> {
        AccountInfo::new(
            Box::leak(Box::new(Pubkey::new_unique())),
            false,
            true,
            Box::leak(Box::new(1)),
            Box::leak(data.into_boxed_slice()),
            Box::leak(Box::new(Pubkey::new_unique())),
            false,
        )
    }

    #[test]
    fn exact_claims_receipt_binds_all_three_post_resources() {
        let claims = Pubkey::new_unique();
        let trading = Pubkey::new_unique();
        let lock = lock_receipt();
        let lock_bytes = lock.encode().expect("lock");
        let request = request(claims, trading, lock, &lock_bytes);
        let request_digest = hash(&request.to_bytes()).to_bytes();
        let mut accounts = (0..33).map(|_| account(Vec::new())).collect::<Vec<_>>();
        accounts[AGGREGATE] = account(vec![1, 2]);
        accounts[POSITION] = account(vec![3, 4]);
        accounts[ADMISSION] = account(vec![5, 6]);
        let aggregate = accounts[AGGREGATE].try_borrow_data().expect("aggregate");
        let position = accounts[POSITION].try_borrow_data().expect("position");
        let admission = accounts[ADMISSION].try_borrow_data().expect("admission");
        let receipt = ClaimsFoundingReceiptV5::new(
            request,
            request_digest,
            hash(&aggregate).to_bytes(),
            hash(&position).to_bytes(),
            hash(&admission).to_bytes(),
            hashv(&[
                CLAIMS_FOUNDING_POST_RESOURCE_DIGEST_DOMAIN_V5,
                &aggregate,
                &position,
                &admission,
            ])
            .to_bytes(),
        )
        .expect("receipt")
        .to_bytes();
        drop(aggregate);
        drop(position);
        drop(admission);
        assert_eq!(
            authenticate_result(request, request_digest, &accounts, claims, claims, &receipt),
            Ok(())
        );
        accounts[POSITION].try_borrow_mut_data().expect("position")[0] ^= 1;
        assert_eq!(
            authenticate_result(request, request_digest, &accounts, claims, claims, &receipt),
            Err(TradingSbfError::Transition.into())
        );
    }

    #[test]
    fn receipt_order_and_substitution_change_the_claims_authority() {
        let claims = Pubkey::new_unique();
        let trading = Pubkey::new_unique();
        let lock = lock_receipt();
        let lock_bytes = lock.encode().expect("lock");
        let realize = realized_receipt(lock);
        let realize_bytes = realize.encode().expect("realize");
        let request = request(claims, trading, lock, &lock_bytes);
        let custody = Pubkey::new_unique();
        assert_eq!(
            authenticate_receipt_join(
                request,
                lock.request_digest,
                &lock_bytes,
                custody,
                &realize_bytes,
                custody,
                trading,
                claims,
            ),
            Ok(())
        );
        assert_eq!(
            authenticate_receipt_join(
                request,
                lock.request_digest,
                &lock_bytes,
                Pubkey::new_unique(),
                &realize_bytes,
                custody,
                trading,
                claims,
            ),
            Err(TradingSbfError::Content.into())
        );
        let mut wrong_realize = realize;
        wrong_realize.context_digest = id(90);
        let wrong_realize = wrong_realize.encode().expect("hostile realize");
        assert_eq!(
            authenticate_receipt_join(
                request,
                lock.request_digest,
                &lock_bytes,
                custody,
                &wrong_realize,
                custody,
                trading,
                claims,
            ),
            Err(TradingSbfError::Content.into())
        );
        let mut substituted = lock;
        substituted.context_digest = id(91);
        let substituted = substituted.encode().expect("hostile lock");
        assert_eq!(
            authenticate_receipt_join(
                request,
                lock.request_digest,
                &substituted,
                custody,
                &realize_bytes,
                custody,
                trading,
                claims,
            ),
            Err(TradingSbfError::Content.into())
        );
    }

    #[test]
    fn route_three_widths_are_exact_and_chain_return_bounded() {
        assert_eq!(CLAIMS_ACCOUNT_COUNT_V4, 32);
        assert_eq!(SERIES_CLAIMS_RECEIPT_DEPENDENCIES_V3.len(), 2);
        assert_eq!(
            CLAIMS_FOUNDING_REQUEST_BYTES_V5
                + PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1
                + PROJECTED_CUSTODY_RECEIPT_BYTES_V1,
            1_472
        );
        assert_eq!(CLAIMS_FOUNDING_RECEIPT_BYTES_V5, 1_008);
    }
}
