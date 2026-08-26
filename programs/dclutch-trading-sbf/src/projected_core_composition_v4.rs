//! Exact Core-Found prefix execution for one admitted projected-Market plan.

extern crate alloc;

use alloc::vec::Vec;

use dclutch_core_contract::ContentId;
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{ProgramV3, ResolvedInvocationV3, ResolvedReceiptDependencyV3, RouteKindV3},
    v4::ProgramV4,
};
use dclutch_market_core_codec::{
    SERIES_CORE_FOUND_ACK_BYTES_V2, SERIES_CORE_FOUND_ACK_MAGIC_V2, SERIES_CORE_REQUEST_BYTES_V1,
    SERIES_CORE_REQUEST_MAGIC_V1, SERIES_FOUND_POST_RESOURCE_DIGEST_DOMAIN_V1,
    SERIES_FOUNDING_PERMIT_BYTES_V1, SeriesCoreActionV1, SeriesCoreRequestV1,
};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_series_v3_kernel::{AccountKeyV3, funding_list_id};
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
        ChildReceiptBankV3, ExpectedReceiptProvenanceV4, append_receipt_dependency_v3,
    },
    projected_custody_composition_v4::AuthenticatedProjectedCustodyPrefixV4,
    projected_market_v2::{
        AuthenticatedFoundSpanV2, ProjectedMarketExecutionV2, authenticate_series_found_span_v2,
    },
    series::{
        artifacts_v3::{
            SERIES_CONSUME_CORE_FOUND_ACCOUNT_BASE_V3, SERIES_CONSUME_LOCK_RECEIPT_BYTES_V3,
        },
        effect_v4::{
            SERIES_CONSUME_CORE_FOUND_PREFIX_ACCOUNT_COUNT_V4, SERIES_CONSUME_FOUND_ROUTE_V4,
            SERIES_CONSUME_LOCK_ROUTE_V4, series_consume_route_account_start_v4,
        },
    },
};

const FOUND_INVOCATION_V4: u32 = 0;
const CALLER: usize = 0;
const MARKET: usize = 1;
const CURRENT_CORE: usize = 19;
const FUNDING_START: usize = 42;
const FOUND_SUFFIX_ACCOUNT_COUNT: usize = 15;
const _: () = assert!(SERIES_CONSUME_CORE_FOUND_PREFIX_ACCOUNT_COUNT_V4 == 42);

/// Exact executed Core-Found prefix fact retained for the common-Hot continuation.
///
/// This fact carries neither a caller-selected route nor a resume token. Its
/// global route and invocation are fixed to `(1, 0)`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedProjectedCorePrefixV4 {
    route: u16,
    invocation: u32,
    raw_request: [u8; SERIES_CORE_REQUEST_BYTES_V1],
    request_digest: [u8; 32],
    raw_acknowledgement: [u8; SERIES_CORE_FOUND_ACK_BYTES_V2],
    producer: Pubkey,
    provenance: ExpectedReceiptProvenanceV4,
    found_span: AuthenticatedFoundSpanV2,
}

impl AuthenticatedProjectedCorePrefixV4 {
    pub(crate) const fn route(&self) -> u16 {
        self.route
    }

    pub(crate) const fn invocation(&self) -> u32 {
        self.invocation
    }

    pub(crate) const fn raw_request(&self) -> &[u8; SERIES_CORE_REQUEST_BYTES_V1] {
        &self.raw_request
    }

    pub(crate) const fn request_digest(&self) -> [u8; 32] {
        self.request_digest
    }

    pub(crate) const fn raw_acknowledgement(&self) -> &[u8; SERIES_CORE_FOUND_ACK_BYTES_V2] {
        &self.raw_acknowledgement
    }

    pub(crate) const fn producer(&self) -> Pubkey {
        self.producer
    }

    pub(crate) const fn provenance(&self) -> ExpectedReceiptProvenanceV4 {
        self.provenance
    }

    pub(crate) const fn found_span(&self) -> AuthenticatedFoundSpanV2 {
        self.found_span
    }

    /// Seed the exact route-one fact into the top-level ephemeral receipt bank.
    pub(crate) fn record_into(self, bank: &mut ChildReceiptBankV3) -> Result<(), ProgramError> {
        bank.record_exact(
            FixedRole::Core,
            self.route,
            self.invocation,
            self.producer,
            self.provenance.context_digest,
            self.provenance.request_kind,
            self.provenance.request_digest,
            SERIES_CORE_FOUND_ACK_MAGIC_V2,
            self.raw_acknowledgement.to_vec(),
        )
    }
}

struct PreparedProjectedCoreFoundV4 {
    invocation: ResolvedInvocationV3,
    request: SeriesCoreRequestV1,
    raw_request: [u8; SERIES_CORE_REQUEST_BYTES_V1],
    request_digest: [u8; 32],
    child_data: Vec<u8>,
    authority_seeds: CallerAuthoritySeedsV1,
    authority_bump: u8,
    funding_count: usize,
}

/// Execute global route one as the exact current-Core Series Found CPI.
///
/// The borrowed proof is selected by Effect V4 from the single authenticated
/// family request. The only appended receipt is the verbatim route-zero return
/// retained by [`AuthenticatedProjectedCustodyPrefixV4`].
#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_projected_core_found_route_v4<'info>(
    program_id: &Pubkey,
    execution: ProjectedMarketExecutionV2<'_>,
    effect: ProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    effect_accounts: &[AccountInfo<'info>],
    request_bank: &[u8],
    core_program: &AccountInfo<'info>,
    lock_prefix: &AuthenticatedProjectedCustodyPrefixV4,
    provenance: ExpectedReceiptProvenanceV4,
) -> Result<AuthenticatedProjectedCorePrefixV4, ProgramError> {
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
        lock_prefix,
        provenance,
    )?;
    let mut child_accounts = invocation_accounts(prepared.invocation, effect_accounts)?;
    let mut metas = Vec::with_capacity(child_accounts.len());
    for (index, account) in child_accounts.iter().enumerate() {
        let signer = index == CALLER;
        metas.push(if account.is_writable {
            AccountMeta::new(*account.key, signer)
        } else {
            AccountMeta::new_readonly(*account.key, signer)
        });
    }
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

    let (producer, return_bytes) = get_return_data().ok_or(TradingSbfError::Transition)?;
    let raw_acknowledgement: [u8; SERIES_CORE_FOUND_ACK_BYTES_V2] = return_bytes
        .as_slice()
        .try_into()
        .map_err(|_| TradingSbfError::Transition)?;
    let found_span = authenticate_found_result(
        execution,
        prepared.request,
        prepared.request_digest,
        prepared.funding_count,
        &child_accounts,
        producer,
        *core_program.key,
        &raw_acknowledgement,
    )?;
    Ok(AuthenticatedProjectedCorePrefixV4 {
        route: SERIES_CONSUME_FOUND_ROUTE_V4,
        invocation: FOUND_INVOCATION_V4,
        raw_request: prepared.raw_request,
        request_digest: prepared.request_digest,
        raw_acknowledgement,
        producer,
        provenance,
        found_span,
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
    effect_accounts: &[AccountInfo<'_>],
    request_bank: &[u8],
    core_program: &AccountInfo<'_>,
    lock_prefix: &AuthenticatedProjectedCustodyPrefixV4,
    provenance: ExpectedReceiptProvenanceV4,
) -> Result<PreparedProjectedCoreFoundV4, ProgramError> {
    let funding_count = usize::from(execution.affine_count());
    let base = effect.base();
    if !core_program.executable
        || core_program.is_signer
        || core_program.is_writable
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
                SERIES_CONSUME_FOUND_ROUTE_V4,
                tail_count,
                scalars,
                identities,
            )
            .map_err(|_| TradingSbfError::Content)?
            != 1
        || provenance.context_digest == [0; 32]
        || provenance.request_kind != SERIES_CORE_REQUEST_MAGIC_V1
        || provenance.request_digest == [0; 32]
    {
        return Err(TradingSbfError::Content.into());
    }
    let resolved = effect
        .resolved_invocation(
            SERIES_CONSUME_FOUND_ROUTE_V4,
            FOUND_INVOCATION_V4,
            tail_count,
            scalars,
            identities,
        )
        .map_err(|_| TradingSbfError::Content)?;
    validate_found_invocation(
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
    let raw_request: [u8; SERIES_CORE_REQUEST_BYTES_V1] = request_bytes
        .try_into()
        .map_err(|_| TradingSbfError::Content)?;
    let request =
        SeriesCoreRequestV1::decode(&raw_request).map_err(|_| TradingSbfError::Content)?;
    if request.action() != SeriesCoreActionV1::Consume {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let borrowed = effect
        .resolved_borrowed_range(SERIES_CONSUME_FOUND_ROUTE_V4, 0, scalars)
        .map_err(|_| TradingSbfError::Content)?;
    let witness = borrowed
        .slice(execution.family_request())
        .map_err(|_| TradingSbfError::Content)?;
    if witness != execution.witness() {
        return Err(TradingSbfError::Content.into());
    }
    let mut child_data = Vec::with_capacity(
        request_bytes
            .len()
            .checked_add(witness.len())
            .and_then(|width| width.checked_add(usize::from(SERIES_CONSUME_LOCK_RECEIPT_BYTES_V3)))
            .ok_or(TradingSbfError::Content)?,
    );
    child_data.extend_from_slice(request_bytes);
    child_data.extend_from_slice(witness);
    append_receipt_dependency_v3(
        resolved.invocation,
        &mut child_data,
        Some(lock_prefix.raw_receipt()),
    )?;
    let request_digest = hash(request_bytes).to_bytes();
    let ticket = request.ticket().ok_or(TradingSbfError::Content)?.to_bytes();
    let market = request.market().ok_or(TradingSbfError::Content)?.to_bytes();
    let authority_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(request.release_set().to_bytes()).map_err(|_| TradingSbfError::Content)?,
        market,
        ExecutionRoleV1::Trading,
        ticket,
        request_digest,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let (expected_authority, authority_bump) =
        Pubkey::find_program_address(&authority_seeds.as_slices(), program_id);
    let child_accounts = invocation_accounts(resolved.invocation, effect_accounts)?;
    if child_accounts
        .get(CALLER)
        .is_none_or(|account| account.key != &expected_authority || !account.is_writable)
        || child_accounts
            .get(CURRENT_CORE)
            .is_none_or(|account| account.key != core_program.key)
    {
        return Err(TradingSbfError::Release.into());
    }
    Ok(PreparedProjectedCoreFoundV4 {
        invocation: resolved.invocation,
        request,
        raw_request,
        request_digest,
        child_data,
        authority_seeds,
        authority_bump,
        funding_count,
    })
}

fn validate_found_invocation(
    effect: ProgramV3<'_>,
    invocation: ResolvedInvocationV3,
    borrowed_range_count: u16,
    funding_count: usize,
) -> Result<(), ProgramError> {
    let funding_count_u16 = u16::try_from(funding_count).map_err(|_| TradingSbfError::Content)?;
    let expected_start =
        series_consume_route_account_start_v4(SERIES_CONSUME_FOUND_ROUTE_V4, funding_count_u16)
            .ok_or(TradingSbfError::Content)?;
    let expected_count = SERIES_CONSUME_CORE_FOUND_ACCOUNT_BASE_V3
        .checked_add(funding_count_u16)
        .ok_or(TradingSbfError::Content)?;
    let dependency = effect
        .resolved_receipt_dependency(invocation.receipt_dependencies, 0)
        .map_err(|_| TradingSbfError::Content)?;
    let expected_dependency = ResolvedReceiptDependencyV3 {
        producer_role: FixedRole::Custody,
        producer_route: SERIES_CONSUME_LOCK_ROUTE_V4,
        producer_invocation: 0,
        expected_receipt_bytes: SERIES_CONSUME_LOCK_RECEIPT_BYTES_V3,
    };
    if invocation.role != FixedRole::Core
        || invocation.kind != RouteKindV3::Once
        || invocation.item.is_some()
        || invocation.fixed_account_start != expected_start
        || invocation.fixed_account_count != expected_count
        || invocation.item_account_count != 0
        || invocation.repeated_item_count != 0
        || invocation.request_len != SERIES_CORE_REQUEST_BYTES_V1
        || borrowed_range_count != 1
        || invocation.receipt_dependencies.len() != 1
        || invocation.receipt_dependency != Some(expected_dependency)
        || dependency != expected_dependency
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

fn invocation_accounts<'info>(
    invocation: ResolvedInvocationV3,
    accounts: &[AccountInfo<'info>],
) -> Result<Vec<AccountInfo<'info>>, ProgramError> {
    let start = usize::from(invocation.fixed_account_start);
    let end = start
        .checked_add(usize::from(invocation.fixed_account_count))
        .ok_or(TradingSbfError::Content)?;
    accounts
        .get(start..end)
        .map(Vec::from)
        .ok_or_else(|| TradingSbfError::Content.into())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_found_result(
    execution: ProjectedMarketExecutionV2<'_>,
    request: SeriesCoreRequestV1,
    request_digest: [u8; 32],
    funding_count: usize,
    accounts: &[AccountInfo<'_>],
    producer: Pubkey,
    core_program: Pubkey,
    raw_acknowledgement: &[u8; SERIES_CORE_FOUND_ACK_BYTES_V2],
) -> Result<AuthenticatedFoundSpanV2, ProgramError> {
    let permit_index = FUNDING_START
        .checked_add(funding_count)
        .ok_or(TradingSbfError::Content)?;
    let expected_len = permit_index
        .checked_add(FOUND_SUFFIX_ACCOUNT_COUNT)
        .ok_or(TradingSbfError::Content)?;
    let market = accounts.get(MARKET).ok_or(TradingSbfError::Content)?;
    let permit = accounts.get(permit_index).ok_or(TradingSbfError::Content)?;
    if accounts.len() != expected_len
        || producer != core_program
        || market.owner != &core_program
        || market.data_is_empty()
        || permit.owner != &core_program
        || permit.data_len() != SERIES_FOUNDING_PERMIT_BYTES_V1
    {
        return Err(TradingSbfError::Transition.into());
    }
    let funding_list_id = ordered_funding_list_id(
        accounts
            .get(FUNDING_START..permit_index)
            .ok_or(TradingSbfError::Content)?,
    )?;
    let market_data = market
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let permit_data = permit
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let post_resource_digest = hashv(&[
        SERIES_FOUND_POST_RESOURCE_DIGEST_DOMAIN_V1,
        &market_data,
        &permit_data,
    ])
    .to_bytes();
    authenticate_series_found_span_v2(
        execution,
        producer.to_bytes(),
        raw_acknowledgement,
        request,
        core_program.to_bytes(),
        permit.key.to_bytes(),
        request_digest,
        funding_list_id,
        post_resource_digest,
    )
    .map_err(|_| TradingSbfError::Transition.into())
}

fn ordered_funding_list_id(accounts: &[AccountInfo<'_>]) -> Result<[u8; 32], ProgramError> {
    let mut keys = Vec::with_capacity(accounts.len());
    for account in accounts {
        keys.push(AccountKeyV3::new(account.key.to_bytes()).map_err(|_| TradingSbfError::Content)?);
    }
    funding_list_id(&keys)
        .map(|identity| identity.to_bytes())
        .map_err(|_| TradingSbfError::Content.into())
}

#[cfg(test)]
mod tests {
    use alloc::{boxed::Box, vec};

    use dclutch_market_core_codec::{Identity, SeriesCoreFoundAckV2};

    use super::*;

    fn identity(byte: u8) -> Identity {
        Identity::new([byte; 32]).expect("nonzero identity")
    }

    fn request() -> SeriesCoreRequestV1 {
        SeriesCoreRequestV1::occurrence(
            SeriesCoreActionV1::Consume,
            identity(1),
            identity(2),
            identity(3),
            identity(4),
            identity(5),
            identity(6),
            identity(7),
            identity(8),
            9,
            10,
            11,
            12,
            13,
            14,
            15,
        )
        .expect("Consume request")
    }

    fn execution(funding_count: u8) -> ProjectedMarketExecutionV2<'static> {
        let mut bytes =
            vec![0_u8; crate::projected_market_v2::PROJECTED_MARKET_EXECUTION_FIXED_BYTES_V2];
        crate::projected_market_v2::encode_projected_market_execution_v2(
            &mut bytes,
            &[9; crate::projected_market_v2::PROJECTED_MARKET_FAMILY_HEADER_BYTES_V2],
            &[],
            funding_count,
        )
        .expect("compact execution");
        ProjectedMarketExecutionV2::decode(Box::leak(bytes.into_boxed_slice())).expect("execution")
    }

    fn account(key: Pubkey, owner: Pubkey, data: Vec<u8>) -> AccountInfo<'static> {
        AccountInfo::new(
            Box::leak(Box::new(key)),
            false,
            false,
            Box::leak(Box::new(1_u64)),
            Box::leak(data.into_boxed_slice()),
            Box::leak(Box::new(owner)),
            false,
        )
    }

    fn found_fixture(
        funding_count: usize,
    ) -> (
        SeriesCoreRequestV1,
        Pubkey,
        Vec<AccountInfo<'static>>,
        [u8; SERIES_CORE_FOUND_ACK_BYTES_V2],
    ) {
        let request = request();
        let core = Pubkey::new_from_array([21; 32]);
        let width = FUNDING_START + funding_count + FOUND_SUFFIX_ACCOUNT_COUNT;
        let mut accounts = Vec::with_capacity(width);
        for index in 0..width {
            let byte = u8::try_from(index + 32).expect("bounded fixture index");
            accounts.push(account(
                Pubkey::new_from_array([byte; 32]),
                Pubkey::new_from_array([99; 32]),
                Vec::new(),
            ));
        }
        accounts[MARKET] = account(Pubkey::new_from_array([22; 32]), core, vec![23; 64]);
        let permit_index = FUNDING_START + funding_count;
        accounts[permit_index] = account(
            Pubkey::new_from_array([24; 32]),
            core,
            vec![25; SERIES_FOUNDING_PERMIT_BYTES_V1],
        );
        let list_id =
            ordered_funding_list_id(&accounts[FUNDING_START..permit_index]).expect("funding list");
        let market_data = accounts[MARKET].try_borrow_data().expect("market data");
        let permit_data = accounts[permit_index]
            .try_borrow_data()
            .expect("permit data");
        let post_resource = hashv(&[
            SERIES_FOUND_POST_RESOURCE_DIGEST_DOMAIN_V1,
            &market_data,
            &permit_data,
        ])
        .to_bytes();
        drop(market_data);
        drop(permit_data);
        let request_bytes = request.encode().expect("request bytes");
        let request_digest = hash(&request_bytes).to_bytes();
        let acknowledgement = SeriesCoreFoundAckV2::new(
            request,
            Identity::new(core.to_bytes()).expect("Core"),
            Identity::new(accounts[permit_index].key.to_bytes()).expect("permit"),
            Identity::new(request_digest).expect("request digest"),
            u8::try_from(funding_count).expect("bounded funding count"),
            Identity::new(list_id).expect("funding list"),
            Identity::new(post_resource).expect("post resource"),
        )
        .expect("Found acknowledgement")
        .encode()
        .expect("acknowledgement bytes");
        (request, core, accounts, acknowledgement)
    }

    #[test]
    fn post_resource_domain_is_owned_by_core_codec() {
        assert_eq!(
            SERIES_FOUND_POST_RESOURCE_DIGEST_DOMAIN_V1,
            b"dclutch/core-series-found-permit/v1"
        );
        assert_ne!(
            hashv(&[
                SERIES_FOUND_POST_RESOURCE_DIGEST_DOMAIN_V1,
                &[1_u8; 8],
                &[2_u8; 8],
            ]),
            hashv(&[
                SERIES_FOUND_POST_RESOURCE_DIGEST_DOMAIN_V1,
                &[1_u8; 8],
                &[3_u8; 8],
            ])
        );
    }

    #[test]
    fn exact_live_market_permit_and_funding_list_promote_once() {
        let funding_count = 2;
        let (request, core, accounts, acknowledgement) = found_fixture(funding_count);
        let request_digest = hash(&request.encode().expect("request bytes")).to_bytes();
        let promoted = authenticate_found_result(
            execution(u8::try_from(funding_count).expect("count")),
            request,
            request_digest,
            funding_count,
            &accounts,
            core,
            core,
            &acknowledgement,
        )
        .expect("authenticated Found span");
        assert_eq!(promoted.funding_count(), 2);
        assert_eq!(
            promoted.acknowledgement_digest(),
            hash(&acknowledgement).to_bytes()
        );
    }

    #[test]
    fn producer_funding_and_postresource_substitution_refuse() {
        let funding_count = 2;
        let request_digest = hash(&request().encode().expect("request bytes")).to_bytes();

        let (request, core, accounts, acknowledgement) = found_fixture(funding_count);
        assert_eq!(
            authenticate_found_result(
                execution(2),
                request,
                request_digest,
                funding_count,
                &accounts,
                Pubkey::new_unique(),
                core,
                &acknowledgement,
            ),
            Err(TradingSbfError::Transition.into())
        );

        let (request, core, mut accounts, acknowledgement) = found_fixture(funding_count);
        accounts[FUNDING_START] = account(Pubkey::new_unique(), Pubkey::new_unique(), Vec::new());
        assert_eq!(
            authenticate_found_result(
                execution(2),
                request,
                request_digest,
                funding_count,
                &accounts,
                core,
                core,
                &acknowledgement,
            ),
            Err(TradingSbfError::Transition.into())
        );

        let (request, core, accounts, acknowledgement) = found_fixture(funding_count);
        let permit_index = FUNDING_START + funding_count;
        accounts[permit_index]
            .try_borrow_mut_data()
            .expect("permit data")[0] ^= 1;
        assert_eq!(
            authenticate_found_result(
                execution(2),
                request,
                request_digest,
                funding_count,
                &accounts,
                core,
                core,
                &acknowledgement,
            ),
            Err(TradingSbfError::Transition.into())
        );
    }
}
