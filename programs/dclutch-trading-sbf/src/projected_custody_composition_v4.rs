//! Family-neutral projected-Custody prefix execution for an admitted Effect V4 plan.

extern crate alloc;

use alloc::vec::Vec;

use dclutch_custody_contract::{
    CUSTODY_AUTHORITY_PDA_DOMAIN_V1, CUSTODY_REPLAY_PDA_DOMAIN_V1,
    PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1, PROJECTED_CUSTODY_LOCK_RECEIPT_MAGIC_V1,
    PROJECTED_CUSTODY_REQUEST_BYTES_V1, PROJECTED_CUSTODY_REQUEST_MAGIC_V1, ProjectedCallerRoleV1,
    ProjectedCustodyCallerSeedsV1, ProjectedCustodyLockReceiptV1, ProjectedCustodyOperationV1,
    ProjectedCustodyPhaseV1, ProjectedCustodyRequestV1, ProjectedCustodyStateSeedsV1,
    ProjectedCustodyStateV1,
};
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{ProgramV3, ResolvedInvocationV3, RouteKindV3},
};
use solana_program::{
    account_info::AccountInfo,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;

use crate::{
    TradingSbfError,
    child_receipt_v3::{ChildReceiptBankV3, ExpectedReceiptProvenanceV4},
    hot_v3::DowngradedEffectAccountsV3,
};

pub(crate) const PROJECTED_CUSTODY_LOCK_ROUTE_V4: u16 = 0;
pub(crate) const PROJECTED_CUSTODY_LOCK_INVOCATION_V4: u32 = 0;

const LOCK_ACCOUNT_COUNT_V4: usize = 14;
const CALLER: usize = 0;
const STATE: usize = 1;
const CACHE: usize = 2;
const REGISTRY: usize = 3;
const CALLER_PROGRAM: usize = 4;
const CALLER_PROGRAMDATA: usize = 5;
const RENT_CREDIT: usize = 6;
const HOARD: usize = 7;
const SOURCE_VAULT: usize = 8;
const CUSTODY_AUTHORITY: usize = 9;
const MINT: usize = 10;
const TOKEN_PROGRAM: usize = 11;
const SOURCE_REPLAY: usize = 12;
const FUTURE_MARKET: usize = 13;

/// Exact executed prefix fact admitted for the ephemeral common-Hot receipt bank.
///
/// This value carries no caller-selected resume point. Its route and invocation
/// are fixed to the global Effect-plan prefix `(0, 0)`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedProjectedCustodyPrefixV4 {
    route: u16,
    invocation: u32,
    request_digest: [u8; 32],
    raw_receipt: [u8; PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1],
    producer: Pubkey,
    provenance: ExpectedReceiptProvenanceV4,
}

struct PreparedProjectedCustodyLockV4<'a> {
    invocation: ResolvedInvocationV3,
    request: ProjectedCustodyRequestV1,
    request_bytes: &'a [u8],
    request_digest: [u8; 32],
}

impl AuthenticatedProjectedCustodyPrefixV4 {
    pub(crate) const fn route(&self) -> u16 {
        self.route
    }

    pub(crate) const fn invocation(&self) -> u32 {
        self.invocation
    }

    pub(crate) const fn request_digest(&self) -> [u8; 32] {
        self.request_digest
    }

    pub(crate) const fn raw_receipt(&self) -> &[u8; PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1] {
        &self.raw_receipt
    }

    pub(crate) const fn producer(&self) -> Pubkey {
        self.producer
    }

    pub(crate) const fn provenance(&self) -> ExpectedReceiptProvenanceV4 {
        self.provenance
    }

    /// Seed the exact route-zero fact into the top-level ephemeral receipt bank.
    pub(crate) fn record_into(self, bank: &mut ChildReceiptBankV3) -> Result<(), ProgramError> {
        bank.record_exact(
            FixedRole::Custody,
            self.route,
            self.invocation,
            self.producer,
            self.provenance.context_digest,
            self.provenance.request_kind,
            self.provenance.request_digest,
            PROJECTED_CUSTODY_LOCK_RECEIPT_MAGIC_V1,
            self.raw_receipt.to_vec(),
        )
    }
}

/// Execute global route zero as the exact projected-Hoard lock/source-close CPI.
///
/// `provenance` must be produced by the common authenticated Effect/request
/// provenance constructor. This adapter never reconstructs a parallel context
/// digest and never accepts a caller-provided resume scalar.
#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_projected_custody_lock_route_v4<'info>(
    program_id: &Pubkey,
    effect: ProgramV3<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    effect_accounts: DowngradedEffectAccountsV3<'_, '_, 'info>,
    request_bank: &[u8],
    custody_program: &AccountInfo<'info>,
    provenance: ExpectedReceiptProvenanceV4,
) -> Result<AuthenticatedProjectedCustodyPrefixV4, ProgramError> {
    let prepared = prepare(
        program_id,
        effect,
        tail_count,
        scalars,
        identities,
        effect_accounts,
        request_bank,
        custody_program,
        provenance,
    )?;
    let child_accounts = invocation_accounts(prepared.invocation, effect_accounts)?;
    let mut metas = Vec::with_capacity(LOCK_ACCOUNT_COUNT_V4);
    for (index, account) in child_accounts.iter().enumerate() {
        let signer = index == CALLER || account.is_signer;
        metas.push(if account.is_writable {
            AccountMeta::new(*account.key, signer)
        } else {
            AccountMeta::new_readonly(*account.key, signer)
        });
    }
    let instruction = Instruction {
        program_id: *custody_program.key,
        accounts: metas,
        data: prepared.request_bytes.to_vec(),
    };
    let mut cpi_accounts = child_accounts;
    cpi_accounts.push(custody_program.clone());
    let caller_seeds =
        ProjectedCustodyCallerSeedsV1::new(prepared.request, prepared.request_digest);
    let (_, bump) = Pubkey::find_program_address(&caller_seeds.as_slices(), program_id);
    let bump_seed = [bump];
    let [domain, release, market, root, context, digest] = caller_seeds.as_slices();
    invoke_signed(
        &instruction,
        &cpi_accounts,
        &[&[domain, release, market, root, context, digest, &bump_seed]],
    )
    .map_err(|_| TradingSbfError::Transition)?;

    let (producer, receipt) = get_return_data().ok_or(TradingSbfError::Transition)?;
    let raw_receipt: [u8; PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1] = receipt
        .as_slice()
        .try_into()
        .map_err(|_| TradingSbfError::Transition)?;
    let poststate = {
        let state_account = cpi_accounts.get(STATE).ok_or(TradingSbfError::Transition)?;
        let state_data = state_account
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Transition)?;
        ProjectedCustodyStateV1::decode(&state_data).map_err(|_| TradingSbfError::Transition)?
    };
    let source_replay = cpi_accounts
        .get(SOURCE_REPLAY)
        .ok_or(TradingSbfError::Transition)?
        .key
        .to_bytes();
    authenticate_lock_result_v4(
        prepared.request,
        prepared.request_digest,
        source_replay,
        raw_receipt,
        poststate,
        producer,
        *custody_program.key,
        provenance,
    )
}

#[allow(clippy::too_many_arguments)]
fn prepare<'a>(
    program_id: &Pubkey,
    effect: ProgramV3<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    effect_accounts: DowngradedEffectAccountsV3<'_, '_, '_>,
    request_bank: &'a [u8],
    custody_program: &AccountInfo<'_>,
    provenance: ExpectedReceiptProvenanceV4,
) -> Result<PreparedProjectedCustodyLockV4<'a>, ProgramError> {
    if !custody_program.executable
        || custody_program.is_signer
        || custody_program.is_writable
        || effect.route_count() < 1
        || effect
            .account_count(tail_count)
            .map_err(|_| TradingSbfError::Content)?
            != effect_accounts.len()
        || effect
            .request_bytes(tail_count)
            .map_err(|_| TradingSbfError::Content)?
            != request_bank.len()
        || effect
            .invocation_count(
                PROJECTED_CUSTODY_LOCK_ROUTE_V4,
                tail_count,
                scalars,
                identities,
            )
            .map_err(|_| TradingSbfError::Content)?
            != 1
    {
        return Err(TradingSbfError::Content.into());
    }
    let invocation = effect
        .resolved_invocation(
            PROJECTED_CUSTODY_LOCK_ROUTE_V4,
            PROJECTED_CUSTODY_LOCK_INVOCATION_V4,
            tail_count,
            scalars,
            identities,
        )
        .map_err(|_| TradingSbfError::Content)?;
    validate_lock_invocation_v4(invocation)?;
    let request_end = invocation
        .request_offset
        .checked_add(invocation.request_len)
        .ok_or(TradingSbfError::Content)?;
    let request_bytes = request_bank
        .get(invocation.request_offset..request_end)
        .ok_or(TradingSbfError::Content)?;
    let request =
        ProjectedCustodyRequestV1::decode(request_bytes).map_err(|_| TradingSbfError::Content)?;
    if request.operation != ProjectedCustodyOperationV1::LockHoardAndCloseSource
        || request.caller_role != ProjectedCallerRoleV1::TradingCapability
        || request.caller_program != program_id.to_bytes()
        || provenance.context_digest == [0; 32]
        || provenance.request_kind != PROJECTED_CUSTODY_REQUEST_MAGIC_V1
        || provenance.request_digest == [0; 32]
    {
        return Err(TradingSbfError::Content.into());
    }
    let request_digest = hash(request_bytes).to_bytes();
    let child_accounts = invocation_accounts(invocation, effect_accounts)?;
    authenticate_lock_frame_v4(
        program_id,
        custody_program,
        request,
        request_digest,
        &child_accounts,
    )?;
    let prestate = {
        let state_account = child_accounts.get(STATE).ok_or(TradingSbfError::Content)?;
        let state_data = state_account
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Content)?;
        ProjectedCustodyStateV1::decode(&state_data).map_err(|_| TradingSbfError::Content)?
    };
    authenticate_lock_prestate_v4(request, prestate)?;
    Ok(PreparedProjectedCustodyLockV4 {
        invocation,
        request,
        request_bytes,
        request_digest,
    })
}

fn validate_lock_invocation_v4(invocation: ResolvedInvocationV3) -> Result<(), ProgramError> {
    if invocation.role != FixedRole::Custody
        || invocation.kind != RouteKindV3::Once
        || invocation.item.is_some()
        || usize::from(invocation.fixed_account_count) != LOCK_ACCOUNT_COUNT_V4
        || invocation.item_account_count != 0
        || invocation.repeated_item_count != 0
        || invocation.request_len != PROJECTED_CUSTODY_REQUEST_BYTES_V1
        || invocation.borrowed_witness.is_some()
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
    if output.len() != LOCK_ACCOUNT_COUNT_V4 {
        return Err(TradingSbfError::Content.into());
    }
    Ok(output)
}

fn authenticate_lock_frame_v4(
    program_id: &Pubkey,
    custody_program: &AccountInfo<'_>,
    request: ProjectedCustodyRequestV1,
    request_digest: [u8; 32],
    accounts: &[AccountInfo<'_>],
) -> Result<(), ProgramError> {
    let state_seeds = ProjectedCustodyStateSeedsV1::from_request(request);
    let expected_state =
        Pubkey::find_program_address(&state_seeds.as_slices(), custody_program.key).0;
    let caller_seeds = ProjectedCustodyCallerSeedsV1::new(request, request_digest);
    let expected_caller = Pubkey::find_program_address(&caller_seeds.as_slices(), program_id).0;
    let expected_source_replay = Pubkey::find_program_address(
        &[
            CUSTODY_REPLAY_PDA_DOMAIN_V1,
            &request.market,
            &request.release_set,
            &request.funding_source_context,
        ],
        custody_program.key,
    )
    .0;
    let expected_authority = Pubkey::find_program_address(
        &[
            CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
            &request.market,
            &request.release_set,
        ],
        custody_program.key,
    )
    .0;
    let expected_keys = [
        Some(expected_caller),
        Some(expected_state),
        None,
        None,
        Some(*program_id),
        None,
        Some(Pubkey::new_from_array(request.rent_credit)),
        Some(Pubkey::new_from_array(request.hoard_vault)),
        Some(Pubkey::new_from_array(request.funding_source_vault)),
        Some(expected_authority),
        Some(Pubkey::new_from_array(request.mint)),
        Some(Pubkey::new_from_array(request.token_program)),
        Some(expected_source_replay),
        Some(Pubkey::new_from_array(request.market)),
    ];
    if accounts.len() != LOCK_ACCOUNT_COUNT_V4
        || accounts
            .iter()
            .any(|account| account.key == custody_program.key)
        || accounts
            .iter()
            .zip(expected_keys)
            .any(|(account, expected)| expected.is_some_and(|key| account.key != &key))
        || !exact_lock_privileges_v4(accounts)
    {
        return Err(TradingSbfError::Content.into());
    }
    let state = accounts.get(STATE).ok_or(TradingSbfError::Content)?;
    let future_market = accounts
        .get(FUTURE_MARKET)
        .ok_or(TradingSbfError::Content)?;
    let source_replay = accounts
        .get(SOURCE_REPLAY)
        .ok_or(TradingSbfError::Content)?;
    if state.owner != custody_program.key
        || future_market.owner != &system_program::ID
        || !future_market.data_is_empty()
        || source_replay.key == state.key
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

fn exact_lock_privileges_v4(accounts: &[AccountInfo<'_>]) -> bool {
    let writable = [
        false, true, false, false, false, false, true, true, true, false, false, false, true, false,
    ];
    let executable = [
        false, false, false, true, true, false, false, false, false, false, false, true, false,
        false,
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

fn authenticate_lock_prestate_v4(
    request: ProjectedCustodyRequestV1,
    state: ProjectedCustodyStateV1,
) -> Result<(), ProgramError> {
    if state.phase != ProjectedCustodyPhaseV1::HoardOpen
        || state.request.operation != ProjectedCustodyOperationV1::OpenHoard
        || state.request.amount != 0
        || state.next_revision != request.expected_revision
        || state.locked_amount != 0
        || state.request.resulting_revision != state.next_revision
        || !same_projection_v4(state.request, request)
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_lock_result_v4(
    request: ProjectedCustodyRequestV1,
    request_digest: [u8; 32],
    source_replay: [u8; 32],
    raw_receipt: [u8; PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1],
    state: ProjectedCustodyStateV1,
    producer: Pubkey,
    expected_producer: Pubkey,
    provenance: ExpectedReceiptProvenanceV4,
) -> Result<AuthenticatedProjectedCustodyPrefixV4, ProgramError> {
    if producer != expected_producer
        || provenance.context_digest == [0; 32]
        || provenance.request_kind != PROJECTED_CUSTODY_REQUEST_MAGIC_V1
        || provenance.request_digest == [0; 32]
        || state.phase != ProjectedCustodyPhaseV1::HoardLocked
        || state.request.operation != ProjectedCustodyOperationV1::LockHoard
        || state.next_revision != request.resulting_revision
        || state.locked_amount != request.amount
        || state.request.amount != request.amount
        || state.request.resulting_revision != request.resulting_revision
        || state.last_request_digest != request_digest
        || !same_projection_v4(state.request, request)
    {
        return Err(TradingSbfError::Transition.into());
    }
    let expected = ProjectedCustodyLockReceiptV1 {
        market: request.market,
        release_set: request.release_set,
        context_digest: request.context_digest,
        source_vault: request.funding_source_vault,
        source_replay,
        hoard_vault: request.hoard_vault,
        rent_credit: request.rent_credit,
        request_digest,
        amount: request.amount,
        source_vault_rent_lamports: request.funding_source_vault_rent_lamports,
        source_replay_rent_lamports: request.funding_source_state_rent_lamports,
        resulting_revision: request.resulting_revision,
    };
    let decoded = ProjectedCustodyLockReceiptV1::decode(&raw_receipt)
        .map_err(|_| TradingSbfError::Transition)?;
    if decoded != expected
        || expected.encode().map_err(|_| TradingSbfError::Transition)? != raw_receipt
    {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(AuthenticatedProjectedCustodyPrefixV4 {
        route: PROJECTED_CUSTODY_LOCK_ROUTE_V4,
        invocation: PROJECTED_CUSTODY_LOCK_INVOCATION_V4,
        request_digest,
        raw_receipt,
        producer,
        provenance,
    })
}

fn same_projection_v4(left: ProjectedCustodyRequestV1, right: ProjectedCustodyRequestV1) -> bool {
    left.caller_role == right.caller_role
        && left.market == right.market
        && left.generation == right.generation
        && left.realm == right.realm
        && left.product_record == right.product_record
        && left.product == right.product
        && left.source == right.source
        && left.release_set == right.release_set
        && left.projection_receipt_digest == right.projection_receipt_digest
        && left.parent_capability_root == right.parent_capability_root
        && left.context_digest == right.context_digest
        && left.caller_program == right.caller_program
        && left.payer == right.payer
        && left.core_program == right.core_program
        && left.rent_program == right.rent_program
        && left.refund_owner == right.refund_owner
        && left.rent_credit == right.rent_credit
        && left.hoard_vault == right.hoard_vault
        && left.funding_source_vault == right.funding_source_vault
        && left.funding_source_context == right.funding_source_context
        && left.funding_source_compartment == right.funding_source_compartment
        && left.mint == right.mint
        && left.token_program == right.token_program
        && left.collateral_release == right.collateral_release
        && left.expiry_slot == right.expiry_slot
        && left.state_rent_lamports == right.state_rent_lamports
        && left.vault_rent_lamports == right.vault_rent_lamports
        && left.funding_source_replay_revision == right.funding_source_replay_revision
        && left.funding_source_state_rent_lamports == right.funding_source_state_rent_lamports
        && left.funding_source_vault_rent_lamports == right.funding_source_vault_rent_lamports
}

#[cfg(test)]
mod tests {
    use dclutch_custody_contract::CompartmentV1;
    use dclutch_effect_kernel::v3::ResolvedReceiptDependenciesV3;

    use super::*;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn lock_request() -> ProjectedCustodyRequestV1 {
        ProjectedCustodyRequestV1 {
            operation: ProjectedCustodyOperationV1::LockHoardAndCloseSource,
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
            expected_revision: 2,
            resulting_revision: 3,
            amount: 24,
            state_rent_lamports: 25,
            vault_rent_lamports: 26,
            funding_source_replay_revision: 27,
            funding_source_state_rent_lamports: 28,
            funding_source_vault_rent_lamports: 29,
        }
    }

    fn open_state(request: ProjectedCustodyRequestV1) -> ProjectedCustodyStateV1 {
        ProjectedCustodyStateV1 {
            phase: ProjectedCustodyPhaseV1::HoardOpen,
            request: ProjectedCustodyRequestV1 {
                operation: ProjectedCustodyOperationV1::OpenHoard,
                expected_revision: 1,
                resulting_revision: 2,
                amount: 0,
                ..request
            },
            next_revision: 2,
            locked_amount: 0,
            last_request_digest: id(30),
            bump: 31,
        }
    }

    fn locked_state(
        request: ProjectedCustodyRequestV1,
        request_digest: [u8; 32],
    ) -> ProjectedCustodyStateV1 {
        let state = ProjectedCustodyStateV1 {
            phase: ProjectedCustodyPhaseV1::HoardLocked,
            request: ProjectedCustodyRequestV1 {
                operation: ProjectedCustodyOperationV1::OpenHoard,
                expected_revision: 1,
                resulting_revision: 2,
                amount: 0,
                ..request
            },
            next_revision: request.resulting_revision,
            locked_amount: request.amount,
            last_request_digest: request_digest,
            bump: 31,
        };
        ProjectedCustodyStateV1::decode(&state.encode().expect("state bytes"))
            .expect("hostile decode")
    }

    fn provenance() -> ExpectedReceiptProvenanceV4 {
        ExpectedReceiptProvenanceV4 {
            context_digest: id(32),
            request_kind: PROJECTED_CUSTODY_REQUEST_MAGIC_V1,
            request_digest: id(33),
        }
    }

    fn receipt(
        request: ProjectedCustodyRequestV1,
        request_digest: [u8; 32],
    ) -> [u8; PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1] {
        ProjectedCustodyLockReceiptV1 {
            market: request.market,
            release_set: request.release_set,
            context_digest: request.context_digest,
            source_vault: request.funding_source_vault,
            source_replay: id(34),
            hoard_vault: request.hoard_vault,
            rent_credit: request.rent_credit,
            request_digest,
            amount: request.amount,
            source_vault_rent_lamports: request.funding_source_vault_rent_lamports,
            source_replay_rent_lamports: request.funding_source_state_rent_lamports,
            resulting_revision: request.resulting_revision,
        }
        .encode()
        .expect("receipt")
    }

    fn invocation() -> ResolvedInvocationV3 {
        ResolvedInvocationV3 {
            role: FixedRole::Custody,
            kind: RouteKindV3::Once,
            item: None,
            fixed_account_start: 5,
            fixed_account_count: 14,
            item_account_start: 0,
            item_account_count: 0,
            item_account_stride: 0,
            repeated_item_count: 0,
            request_offset: 0,
            request_len: PROJECTED_CUSTODY_REQUEST_BYTES_V1,
            borrowed_witness: None,
            receipt_dependencies: ResolvedReceiptDependenciesV3::empty(),
            receipt_dependency: None,
        }
    }

    #[test]
    fn exact_prestate_and_once_geometry_accept() {
        let request = lock_request();
        assert_eq!(validate_lock_invocation_v4(invocation()), Ok(()));
        assert_eq!(
            authenticate_lock_prestate_v4(request, open_state(request)),
            Ok(())
        );

        let mut hostile = invocation();
        hostile.fixed_account_count = 13;
        assert_eq!(
            validate_lock_invocation_v4(hostile),
            Err(TradingSbfError::Content.into())
        );
        let mut hostile_state = open_state(request);
        hostile_state.next_revision = 3;
        assert_eq!(
            authenticate_lock_prestate_v4(request, hostile_state),
            Err(TradingSbfError::Content.into())
        );
    }

    #[test]
    fn exact_receipt_and_poststate_seed_one_prefix_fact() {
        let request = lock_request();
        let request_digest = hash(&request.encode().expect("request")).to_bytes();
        let producer = Pubkey::new_from_array(id(35));
        let fact = authenticate_lock_result_v4(
            request,
            request_digest,
            id(34),
            receipt(request, request_digest),
            locked_state(request, request_digest),
            producer,
            producer,
            provenance(),
        )
        .expect("prefix fact");
        assert_eq!(fact.route(), 0);
        assert_eq!(fact.invocation(), 0);
        assert_eq!(fact.request_digest(), request_digest);
        assert_eq!(fact.producer(), producer);
        assert_eq!(fact.provenance(), provenance());
        assert_eq!(fact.raw_receipt(), &receipt(request, request_digest));
        let mut bank = ChildReceiptBankV3::new();
        fact.record_into(&mut bank).expect("receipt bank");
    }

    #[test]
    fn receipt_producer_poststate_and_provenance_substitution_refuse() {
        let request = lock_request();
        let request_digest = hash(&request.encode().expect("request")).to_bytes();
        let producer = Pubkey::new_from_array(id(35));
        let canonical_receipt = receipt(request, request_digest);
        let canonical_state = locked_state(request, request_digest);
        let canonical_provenance = provenance();

        let mut wrong_receipt = canonical_receipt;
        *wrong_receipt.get_mut(288).expect("amount byte") ^= 1;
        let mut wrong_state = canonical_state;
        wrong_state.last_request_digest = id(36);
        for result in [
            authenticate_lock_result_v4(
                request,
                request_digest,
                id(34),
                wrong_receipt,
                canonical_state,
                producer,
                producer,
                canonical_provenance,
            ),
            authenticate_lock_result_v4(
                request,
                request_digest,
                id(34),
                canonical_receipt,
                wrong_state,
                producer,
                producer,
                canonical_provenance,
            ),
            authenticate_lock_result_v4(
                request,
                request_digest,
                id(34),
                canonical_receipt,
                canonical_state,
                Pubkey::new_unique(),
                producer,
                canonical_provenance,
            ),
            authenticate_lock_result_v4(
                request,
                request_digest,
                id(34),
                canonical_receipt,
                canonical_state,
                producer,
                producer,
                ExpectedReceiptProvenanceV4 {
                    request_digest: [0; 32],
                    ..canonical_provenance
                },
            ),
        ] {
            assert_eq!(result, Err(TradingSbfError::Transition.into()));
        }
    }
}
