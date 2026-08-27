//! Exact projected-Hoard realization for an admitted Effect V4 plan.

extern crate alloc;

use alloc::vec::Vec;

use dclutch_custody_contract::{
    CUSTODY_AUTHORITY_PDA_DOMAIN_V1, CUSTODY_REPLAY_BYTES_V1, CustodyReplayV1,
    PROJECTED_CUSTODY_RECEIPT_BYTES_V1, PROJECTED_CUSTODY_RECEIPT_MAGIC_V1,
    PROJECTED_CUSTODY_REQUEST_BYTES_V1, PROJECTED_CUSTODY_REQUEST_MAGIC_V1,
    PROJECTED_CUSTODY_STATE_BYTES_V1, ProjectedCallerRoleV1, ProjectedCustodyCallerSeedsV1,
    ProjectedCustodyOperationV1, ProjectedCustodyReceiptV1, ProjectedCustodyRequestV1,
    ProjectedCustodyStateSeedsV1, ProjectedCustodyStateV1, normal_replay_from_realization_v1,
};
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{ResolvedInvocationV3, RouteKindV3},
    v4::ProgramV4,
};
use dclutch_market_core_codec::{CoreState, STATE_BYTES, SeriesCoreActionV1, SeriesCoreRequestV1};
use solana_program::{
    account_info::AccountInfo,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::{
    TradingSbfError,
    child_receipt_v3::{ChildReceiptBankV3, ExpectedReceiptProvenanceV4},
    hot_v3::DowngradedEffectAccountsV3,
    projected_core_composition_v4::AuthenticatedProjectedCorePrefixV4,
    projected_custody_composition_v4::AuthenticatedProjectedCustodyPrefixV4,
    projected_market_v2::ProjectedMarketExecutionV2,
    series::effect_v4::{SERIES_CONSUME_REALIZE_ROUTE_V4, series_consume_route_account_start_v4},
};

const REALIZE_INVOCATION_V4: u32 = 0;
const REALIZE_ACCOUNT_COUNT_V4: usize = 12;
const CALLER: usize = 0;
const STATE: usize = 1;
const RENT_CREDIT: usize = 6;
const HOARD: usize = 7;
const MARKET: usize = 8;
const CUSTODY_AUTHORITY: usize = 9;
const MINT: usize = 10;
const TOKEN_PROGRAM: usize = 11;

/// Exact route-two realization fact retained for Claims Founding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedProjectedRealizeV4 {
    route: u16,
    invocation: u32,
    raw_request: [u8; PROJECTED_CUSTODY_REQUEST_BYTES_V1],
    request_digest: [u8; 32],
    raw_receipt: [u8; PROJECTED_CUSTODY_RECEIPT_BYTES_V1],
    producer: Pubkey,
    provenance: ExpectedReceiptProvenanceV4,
}

impl AuthenticatedProjectedRealizeV4 {
    pub(crate) const fn route(&self) -> u16 {
        self.route
    }

    pub(crate) const fn invocation(&self) -> u32 {
        self.invocation
    }

    pub(crate) const fn raw_request(&self) -> &[u8; PROJECTED_CUSTODY_REQUEST_BYTES_V1] {
        &self.raw_request
    }

    pub(crate) const fn request_digest(&self) -> [u8; 32] {
        self.request_digest
    }

    pub(crate) const fn raw_receipt(&self) -> &[u8; PROJECTED_CUSTODY_RECEIPT_BYTES_V1] {
        &self.raw_receipt
    }

    pub(crate) const fn producer(&self) -> Pubkey {
        self.producer
    }

    pub(crate) const fn provenance(&self) -> ExpectedReceiptProvenanceV4 {
        self.provenance
    }

    /// Seed the exact route-two result into the ephemeral receipt bank.
    pub(crate) fn record_into(self, bank: &mut ChildReceiptBankV3) -> Result<(), ProgramError> {
        bank.record_exact(
            FixedRole::Custody,
            self.route,
            self.invocation,
            self.producer,
            self.provenance.context_digest,
            self.provenance.request_kind,
            self.provenance.request_digest,
            PROJECTED_CUSTODY_RECEIPT_MAGIC_V1,
            self.raw_receipt.to_vec(),
        )
    }
}

struct PreparedProjectedRealizeV4 {
    invocation: ResolvedInvocationV3,
    raw_request: [u8; PROJECTED_CUSTODY_REQUEST_BYTES_V1],
    request_digest: [u8; 32],
    expected_receipt: ProjectedCustodyReceiptV1,
    expected_replay: CustodyReplayV1,
    authority_seeds: ProjectedCustodyCallerSeedsV1,
    authority_bump: u8,
    state_lamports: u64,
    rent_credit_lamports: u64,
    market_digest: [u8; 32],
    hoard_digest: [u8; 32],
    hoard_lamports: u64,
}

/// Execute global route two as a no-token-move projected-Hoard realization.
#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_projected_realize_route_v4<'info>(
    program_id: &Pubkey,
    execution: ProjectedMarketExecutionV2<'_>,
    effect: ProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    effect_accounts: DowngradedEffectAccountsV3<'_, '_, 'info>,
    request_bank: &[u8],
    custody_program: &AccountInfo<'info>,
    lock_prefix: &AuthenticatedProjectedCustodyPrefixV4,
    core_prefix: &AuthenticatedProjectedCorePrefixV4,
    provenance: ExpectedReceiptProvenanceV4,
) -> Result<AuthenticatedProjectedRealizeV4, ProgramError> {
    let prepared = prepare(
        program_id,
        execution,
        effect,
        tail_count,
        scalars,
        identities,
        effect_accounts,
        request_bank,
        custody_program,
        lock_prefix,
        core_prefix,
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
        program_id: *custody_program.key,
        accounts: metas,
        data: prepared.raw_request.to_vec(),
    };
    child_accounts.push(custody_program.clone());
    let bump_seed = [prepared.authority_bump];
    let [domain, release, market, root, context, digest] = prepared.authority_seeds.as_slices();
    invoke_signed(
        &instruction,
        &child_accounts,
        &[&[domain, release, market, root, context, digest, &bump_seed]],
    )
    .map_err(|_| TradingSbfError::Transition)?;
    let (producer, return_bytes) = get_return_data().ok_or(TradingSbfError::Transition)?;
    let raw_receipt: [u8; PROJECTED_CUSTODY_RECEIPT_BYTES_V1] = return_bytes
        .as_slice()
        .try_into()
        .map_err(|_| TradingSbfError::Transition)?;
    authenticate_result(
        &prepared,
        &child_accounts,
        producer,
        *custody_program.key,
        raw_receipt,
    )?;
    Ok(AuthenticatedProjectedRealizeV4 {
        route: SERIES_CONSUME_REALIZE_ROUTE_V4,
        invocation: REALIZE_INVOCATION_V4,
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
    execution: ProjectedMarketExecutionV2<'_>,
    effect: ProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    effect_accounts: DowngradedEffectAccountsV3<'_, '_, '_>,
    request_bank: &[u8],
    custody_program: &AccountInfo<'_>,
    lock_prefix: &AuthenticatedProjectedCustodyPrefixV4,
    core_prefix: &AuthenticatedProjectedCorePrefixV4,
    provenance: ExpectedReceiptProvenanceV4,
) -> Result<PreparedProjectedRealizeV4, ProgramError> {
    let funding_count = u16::from(execution.affine_count());
    let base = effect.base();
    if !custody_program.executable
        || custody_program.is_signer
        || custody_program.is_writable
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
                SERIES_CONSUME_REALIZE_ROUTE_V4,
                tail_count,
                scalars,
                identities,
            )
            .map_err(|_| TradingSbfError::Content)?
            != 1
        || core_prefix.found_span().funding_count() != execution.affine_count()
        || lock_prefix.route() != 0
        || lock_prefix.invocation() != 0
        || core_prefix.route() != 1
        || core_prefix.invocation() != 0
        || provenance.context_digest == [0; 32]
        || provenance.request_kind != PROJECTED_CUSTODY_REQUEST_MAGIC_V1
        || provenance.request_digest == [0; 32]
    {
        return Err(TradingSbfError::Content.into());
    }
    let resolved = effect
        .resolved_invocation(
            SERIES_CONSUME_REALIZE_ROUTE_V4,
            REALIZE_INVOCATION_V4,
            tail_count,
            scalars,
            identities,
        )
        .map_err(|_| TradingSbfError::Content)?;
    validate_invocation(
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
    let raw_request: [u8; PROJECTED_CUSTODY_REQUEST_BYTES_V1] = request_bytes
        .try_into()
        .map_err(|_| TradingSbfError::Content)?;
    let request =
        ProjectedCustodyRequestV1::decode(&raw_request).map_err(|_| TradingSbfError::Content)?;
    if request.operation != ProjectedCustodyOperationV1::RealizeAndClose
        || request.caller_role != ProjectedCallerRoleV1::TradingCapability
        || request.caller_program != program_id.to_bytes()
    {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    authenticate_prefix_join(request, lock_prefix, core_prefix)?;
    let request_digest = hash(request_bytes).to_bytes();
    let child_accounts = invocation_accounts(resolved.invocation, effect_accounts)?;
    authenticate_frame(
        program_id,
        custody_program,
        request,
        request_digest,
        &child_accounts,
    )?;
    let state_account = account_at(&child_accounts, STATE)?;
    let rent_credit_account = account_at(&child_accounts, RENT_CREDIT)?;
    let hoard_account = account_at(&child_accounts, HOARD)?;
    let market_account = account_at(&child_accounts, MARKET)?;
    let state_data = state_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let state =
        ProjectedCustodyStateV1::decode(&state_data).map_err(|_| TradingSbfError::Content)?;
    drop(state_data);
    let market_data = market_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let market_state = CoreState::decode(&market_data).map_err(|_| TradingSbfError::Content)?;
    let market_digest = hash(&market_data).to_bytes();
    drop(market_data);
    let expected_receipt = state
        .realize_and_close(
            request,
            request_digest,
            market_state,
            market_digest,
            request.amount,
            rent_credit_account.key.to_bytes(),
        )
        .map_err(|_| TradingSbfError::Content)?;
    let expected_receipt_bytes = expected_receipt
        .encode()
        .map_err(|_| TradingSbfError::Content)?;
    let expected_replay = normal_replay_from_realization_v1(
        state,
        expected_receipt,
        hash(&expected_receipt_bytes).to_bytes(),
    )
    .map_err(|_| TradingSbfError::Content)?;
    let hoard_data = hoard_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let hoard_digest = hash(&hoard_data).to_bytes();
    drop(hoard_data);
    let authority_seeds = ProjectedCustodyCallerSeedsV1::new(request, request_digest);
    let (_, authority_bump) =
        Pubkey::find_program_address(&authority_seeds.as_slices(), program_id);
    Ok(PreparedProjectedRealizeV4 {
        invocation: resolved.invocation,
        raw_request,
        request_digest,
        expected_receipt,
        expected_replay,
        authority_seeds,
        authority_bump,
        state_lamports: state_account.lamports(),
        rent_credit_lamports: rent_credit_account.lamports(),
        market_digest,
        hoard_digest,
        hoard_lamports: hoard_account.lamports(),
    })
}

fn validate_invocation(
    invocation: ResolvedInvocationV3,
    borrowed_range_count: u16,
    funding_count: u16,
) -> Result<(), ProgramError> {
    let expected_start =
        series_consume_route_account_start_v4(SERIES_CONSUME_REALIZE_ROUTE_V4, funding_count)
            .ok_or(TradingSbfError::Content)?;
    if invocation.role != FixedRole::Custody
        || invocation.kind != RouteKindV3::Once
        || invocation.item.is_some()
        || invocation.fixed_account_start != expected_start
        || usize::from(invocation.fixed_account_count) != REALIZE_ACCOUNT_COUNT_V4
        || invocation.item_account_count != 0
        || invocation.repeated_item_count != 0
        || invocation.request_len != PROJECTED_CUSTODY_REQUEST_BYTES_V1
        || borrowed_range_count != 0
        || !invocation.receipt_dependencies.is_empty()
        || invocation.receipt_dependency.is_some()
    {
        Err(TradingSbfError::Content.into())
    } else {
        Ok(())
    }
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
    if output.len() != REALIZE_ACCOUNT_COUNT_V4 {
        return Err(TradingSbfError::Content.into());
    }
    Ok(output)
}

fn authenticate_prefix_join(
    request: ProjectedCustodyRequestV1,
    lock_prefix: &AuthenticatedProjectedCustodyPrefixV4,
    core_prefix: &AuthenticatedProjectedCorePrefixV4,
) -> Result<(), ProgramError> {
    let lock =
        dclutch_custody_contract::ProjectedCustodyLockReceiptV1::decode(lock_prefix.raw_receipt())
            .map_err(|_| TradingSbfError::Content)?;
    let core = SeriesCoreRequestV1::decode(core_prefix.raw_request())
        .map_err(|_| TradingSbfError::Content)?;
    if core.action() != SeriesCoreActionV1::Consume
        || core
            .market()
            .is_none_or(|market| market.to_bytes() != request.market)
        || core.market_generation() != Some(request.generation)
        || core.release_set().to_bytes() != request.release_set
        || core.hoard_principal() != request.amount
        || core_prefix.producer().to_bytes() != request.core_program
        || lock.market != request.market
        || lock.release_set != request.release_set
        || lock.context_digest != request.context_digest
        || lock.hoard_vault != request.hoard_vault
        || lock.rent_credit != request.rent_credit
        || lock.amount != request.amount
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

fn authenticate_frame(
    program_id: &Pubkey,
    custody_program: &AccountInfo<'_>,
    request: ProjectedCustodyRequestV1,
    request_digest: [u8; 32],
    accounts: &[AccountInfo<'_>],
) -> Result<(), ProgramError> {
    let state_account = account_at(accounts, STATE)?;
    let market_account = account_at(accounts, MARKET)?;
    let state = Pubkey::find_program_address(
        &ProjectedCustodyStateSeedsV1::from_request(request).as_slices(),
        custody_program.key,
    )
    .0;
    let caller = Pubkey::find_program_address(
        &ProjectedCustodyCallerSeedsV1::new(request, request_digest).as_slices(),
        program_id,
    )
    .0;
    let authority = Pubkey::find_program_address(
        &[
            CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
            &request.market,
            &request.release_set,
        ],
        custody_program.key,
    )
    .0;
    let expected = [
        Some(caller),
        Some(state),
        None,
        None,
        Some(*program_id),
        None,
        Some(Pubkey::new_from_array(request.rent_credit)),
        Some(Pubkey::new_from_array(request.hoard_vault)),
        Some(Pubkey::new_from_array(request.market)),
        Some(authority),
        Some(Pubkey::new_from_array(request.mint)),
        Some(Pubkey::new_from_array(request.token_program)),
    ];
    if accounts.len() != REALIZE_ACCOUNT_COUNT_V4
        || accounts
            .iter()
            .any(|account| account.key == custody_program.key)
        || accounts
            .iter()
            .zip(expected)
            .any(|(account, key)| key.is_some_and(|key| account.key != &key))
        || !exact_privileges(accounts)
        || state_account.owner != custody_program.key
        || state_account.data_len() != PROJECTED_CUSTODY_STATE_BYTES_V1
        || market_account.owner.to_bytes() != request.core_program
        || market_account.data_len() != STATE_BYTES
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

fn exact_privileges(accounts: &[AccountInfo<'_>]) -> bool {
    let writable = [
        false, true, false, false, false, false, false, false, false, false, false, false,
    ];
    let executable = [
        false, false, false, true, true, false, false, false, false, false, false, true,
    ];
    accounts
        .iter()
        .zip(writable)
        .zip(executable)
        .all(|((account, writable), executable)| {
            !account.is_signer
                && account.is_writable == writable
                && account.executable == executable
        })
}

fn authenticate_result(
    prepared: &PreparedProjectedRealizeV4,
    accounts: &[AccountInfo<'_>],
    producer: Pubkey,
    custody_program: Pubkey,
    raw_receipt: [u8; PROJECTED_CUSTODY_RECEIPT_BYTES_V1],
) -> Result<(), ProgramError> {
    let state_account = account_at(accounts, STATE)?;
    let rent_credit_account = account_at(accounts, RENT_CREDIT)?;
    let hoard_account = account_at(accounts, HOARD)?;
    let market_account = account_at(accounts, MARKET)?;
    let receipt =
        ProjectedCustodyReceiptV1::decode(&raw_receipt).map_err(|_| TradingSbfError::Transition)?;
    let state_data = state_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let replay = CustodyReplayV1::decode(&state_data).map_err(|_| TradingSbfError::Transition)?;
    let replay_bytes = prepared
        .expected_replay
        .to_bytes()
        .map_err(|_| TradingSbfError::Transition)?;
    let market_data = market_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let market_digest = hash(&market_data).to_bytes();
    let hoard_data = hoard_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let hoard_digest = hash(&hoard_data).to_bytes();
    if producer != custody_program
        || receipt != prepared.expected_receipt
        || prepared
            .expected_receipt
            .encode()
            .map_err(|_| TradingSbfError::Transition)?
            != raw_receipt
        || replay != prepared.expected_replay
        || state_data.as_ref() != replay_bytes
        || state_account.owner != &custody_program
        || state_account.data_len() != CUSTODY_REPLAY_BYTES_V1
        || state_account.lamports() != prepared.state_lamports
        || rent_credit_account.lamports() != prepared.rent_credit_lamports
        || market_digest != prepared.market_digest
        || hoard_account.lamports() != prepared.hoard_lamports
        || hoard_digest != prepared.hoard_digest
    {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(())
}

fn account_at<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    accounts.get(index).ok_or(TradingSbfError::Content.into())
}

#[cfg(test)]
mod tests {
    use alloc::{boxed::Box, vec};

    use dclutch_custody_contract::{CallerRoleV1, CompartmentV1};
    use dclutch_effect_kernel::v3::ResolvedReceiptDependenciesV3;

    use super::*;

    fn id(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn request() -> ProjectedCustodyRequestV1 {
        ProjectedCustodyRequestV1 {
            operation: ProjectedCustodyOperationV1::RealizeAndClose,
            caller_role: ProjectedCallerRoleV1::TradingCapability,
            market: id(1),
            generation: 2,
            realm: id(3),
            product_record: id(4),
            product: id(5),
            source: id(6),
            release_set: id(7),
            projection_receipt_digest: id(8),
            parent_capability_root: id(9),
            context_digest: id(10),
            caller_program: id(11),
            payer: id(12),
            core_program: id(13),
            rent_program: id(14),
            refund_owner: id(15),
            rent_credit: id(16),
            hoard_vault: id(17),
            funding_source_vault: id(18),
            funding_source_context: id(19),
            funding_source_compartment: CompartmentV1::SeriesEscrow,
            mint: id(20),
            token_program: id(21),
            collateral_release: id(22),
            expiry_slot: 23,
            expected_revision: 3,
            resulting_revision: 4,
            amount: 25,
            state_rent_lamports: 26,
            vault_rent_lamports: 27,
            funding_source_replay_revision: 28,
            funding_source_state_rent_lamports: 29,
            funding_source_vault_rent_lamports: 30,
        }
    }

    fn account(key: Pubkey, owner: Pubkey, lamports: u64, data: Vec<u8>) -> AccountInfo<'static> {
        AccountInfo::new(
            Box::leak(Box::new(key)),
            false,
            false,
            Box::leak(Box::new(lamports)),
            Box::leak(data.into_boxed_slice()),
            Box::leak(Box::new(owner)),
            false,
        )
    }

    fn result_fixture() -> (
        PreparedProjectedRealizeV4,
        Vec<AccountInfo<'static>>,
        Pubkey,
        [u8; PROJECTED_CUSTODY_RECEIPT_BYTES_V1],
    ) {
        let request = request();
        let custody = Pubkey::new_from_array(id(31));
        let market_data = vec![32; STATE_BYTES];
        let market_digest = hash(&market_data).to_bytes();
        let receipt = ProjectedCustodyReceiptV1 {
            realized: true,
            aborted_open: false,
            market: request.market,
            release_set: request.release_set,
            parent_capability_root: request.parent_capability_root,
            context_digest: request.context_digest,
            hoard_vault: request.hoard_vault,
            amount: request.amount,
            request_digest: id(33),
            market_state_digest: market_digest,
            rent_credit: request.rent_credit,
            resulting_revision: request.resulting_revision,
        };
        let raw_receipt = receipt.encode().expect("receipt");
        let replay = CustodyReplayV1 {
            caller_role: CallerRoleV1::Trading,
            release_set: request.release_set,
            market: request.market,
            realm: request.realm,
            context: request.context_digest,
            caller_program: request.caller_program,
            rent_refund: request.rent_credit,
            open_vault_count: 1,
            next_revision: 1,
            generation: request.generation,
            last_request_digest: receipt.request_digest,
            last_poststate_commitment: hash(&raw_receipt).to_bytes(),
        };
        let hoard_data = vec![34; 165];
        let mut accounts = Vec::with_capacity(REALIZE_ACCOUNT_COUNT_V4);
        for index in 0..REALIZE_ACCOUNT_COUNT_V4 {
            let byte = u8::try_from(index + 40).expect("bounded index");
            accounts.push(account(
                Pubkey::new_from_array([byte; 32]),
                Pubkey::new_from_array(id(60)),
                1,
                Vec::new(),
            ));
        }
        accounts[STATE] = account(
            Pubkey::new_from_array(id(61)),
            custody,
            100,
            replay.to_bytes().expect("replay").to_vec(),
        );
        accounts[RENT_CREDIT] = account(
            Pubkey::new_from_array(request.rent_credit),
            Pubkey::new_from_array(request.rent_program),
            200,
            vec![62; 32],
        );
        accounts[HOARD] = account(
            Pubkey::new_from_array(request.hoard_vault),
            Pubkey::new_from_array(request.token_program),
            300,
            hoard_data.clone(),
        );
        accounts[MARKET] = account(
            Pubkey::new_from_array(request.market),
            Pubkey::new_from_array(request.core_program),
            400,
            market_data,
        );
        let raw_request = request.encode().expect("request");
        let request_digest = hash(&raw_request).to_bytes();
        let seeds = ProjectedCustodyCallerSeedsV1::new(request, request_digest);
        let prepared = PreparedProjectedRealizeV4 {
            invocation: ResolvedInvocationV3 {
                role: FixedRole::Custody,
                kind: RouteKindV3::Once,
                item: None,
                fixed_account_start: 0,
                fixed_account_count: 12,
                item_account_start: 0,
                item_account_count: 0,
                item_account_stride: 0,
                repeated_item_count: 0,
                request_offset: 0,
                request_len: PROJECTED_CUSTODY_REQUEST_BYTES_V1,
                borrowed_witness: None,
                receipt_dependencies: ResolvedReceiptDependenciesV3::empty(),
                receipt_dependency: None,
            },
            raw_request,
            request_digest,
            expected_receipt: receipt,
            expected_replay: replay,
            authority_seeds: seeds,
            authority_bump: 1,
            state_lamports: 100,
            rent_credit_lamports: 200,
            market_digest,
            hoard_digest: hash(&hoard_data).to_bytes(),
            hoard_lamports: 300,
        };
        (prepared, accounts, custody, raw_receipt)
    }

    #[test]
    fn realization_route_and_receipt_are_not_reindexed() {
        assert_eq!(SERIES_CONSUME_REALIZE_ROUTE_V4, 2);
        assert_eq!(REALIZE_INVOCATION_V4, 0);
        assert_eq!(PROJECTED_CUSTODY_RECEIPT_BYTES_V1, 320);
        assert_eq!(PROJECTED_CUSTODY_RECEIPT_MAGIC_V1, *b"DCLPCR01");
    }

    #[test]
    fn exact_receipt_replay_and_no_move_poststate_accept() {
        let (prepared, accounts, custody, receipt) = result_fixture();
        assert_eq!(
            authenticate_result(&prepared, &accounts, custody, custody, receipt),
            Ok(())
        );
    }

    #[test]
    fn producer_receipt_replay_rent_market_and_hoard_substitution_refuse() {
        let (prepared, accounts, custody, receipt) = result_fixture();
        assert_eq!(
            authenticate_result(&prepared, &accounts, Pubkey::new_unique(), custody, receipt),
            Err(TradingSbfError::Transition.into())
        );

        let (prepared, accounts, custody, mut receipt) = result_fixture();
        *receipt.get_mut(288).expect("amount byte") ^= 1;
        assert_eq!(
            authenticate_result(&prepared, &accounts, custody, custody, receipt),
            Err(TradingSbfError::Transition.into())
        );

        let (prepared, accounts, custody, receipt) = result_fixture();
        accounts[STATE].try_borrow_mut_data().expect("replay data")[16] ^= 1;
        assert_eq!(
            authenticate_result(&prepared, &accounts, custody, custody, receipt),
            Err(TradingSbfError::Transition.into())
        );

        let (prepared, accounts, custody, receipt) = result_fixture();
        **accounts[RENT_CREDIT]
            .try_borrow_mut_lamports()
            .expect("rent lamports") += 1;
        assert_eq!(
            authenticate_result(&prepared, &accounts, custody, custody, receipt),
            Err(TradingSbfError::Transition.into())
        );

        let (prepared, accounts, custody, receipt) = result_fixture();
        accounts[MARKET].try_borrow_mut_data().expect("market data")[0] ^= 1;
        assert_eq!(
            authenticate_result(&prepared, &accounts, custody, custody, receipt),
            Err(TradingSbfError::Transition.into())
        );

        let (prepared, accounts, custody, receipt) = result_fixture();
        accounts[HOARD].try_borrow_mut_data().expect("Hoard data")[0] ^= 1;
        assert_eq!(
            authenticate_result(&prepared, &accounts, custody, custody, receipt),
            Err(TradingSbfError::Transition.into())
        );
    }
}
