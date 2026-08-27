#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(unexpected_cfgs)]

//! Reusable successor Dealer family handler and thin measurement adapter.
//!
//! Immutable curves remain runtime data interpreted by
//! `dclutch-dealer-codec`. This layer owns only Solana account authentication,
//! Registry reauthentication, signatures, Clock binding, child CPI composition,
//! immediate receipt/postcondition checks, and commit-last persistence. The
//! public family handler is intended for the one canonical data-driven Trading
//! controller. The crate-local entrypoint only measures the same handler as a
//! standalone prototype; it is not a second accepted Trading release identity.

extern crate alloc;
extern crate std;

mod frame;

use alloc::vec::Vec;
use dclutch_claims_svm::{
    CLAIMS_PLAN_HEADER_BYTES_V1, CallerRole, ClaimsAction, ClaimsAggregateSeedsV1, ClaimsPlanV1,
    ClaimsPositionSeedsV1, ClaimsReceiptV1, NO_POSITION_REVISION,
};
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    CUSTODY_AUTHORITY_PDA_DOMAIN_V1, CallerRoleV1, CompartmentV1, ContextV1,
    CustodyAuthoritySeedsV1, CustodyReceiptV1, CustodyReplayV1, CustodyRequestV1,
    CustodyVaultSeedsV1, OperationV1,
};
use dclutch_dealer_codec::{
    Action, CANDIDATE_BYTES, CandidateView, ClaimAction, CustodyRole, Inputs, MAX_OUTCOMES,
    NowDisciplineV1, POLICY_BYTES, Plan, Policy, RECEIPT_BYTES, REQUEST_BYTES, ReleaseReceipt,
    Request, STATE_BYTES, Side, State, interpret,
};
use dclutch_economic_slice_kernel::{
    Phase as ClaimsPhase, market_identity, market_outcome_count, market_phase,
    market_registry_program, market_release_set_id, market_revision, position_market_id,
    position_owner, position_revision,
};
use dclutch_market_core_codec::{CoreState, MarketCoreStateSeedsV2, Phase as CorePhase};
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_PDA_DOMAIN, RealmV1,
};
use dclutch_registry_activation_auth_v1::authenticate_activated_role_v1;
use dclutch_registry_svm::AuthenticatedRoleReceiptV1;
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_token_svm::{
    COption, ExactTransferProfileV1, PRODUCTION_ADAPTER_RELEASES, TokenAccount,
};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::sysvar;

use frame::{ChildResources, DYNAMIC_CUSTODY_ROLES, custody_role_slot};

/// Exact common accounts before plan-derived Claims/Custody resources.
pub const COMMON_ACCOUNT_COUNT_V1: usize = 23;
/// PDA domain for immutable Dealer policy accounts.
pub const POLICY_PDA_DOMAIN_V1: &[u8] = b"dclutch:dealer-policy:v1";
/// PDA domain for immutable Dealer Candidate accounts.
pub const CANDIDATE_PDA_DOMAIN_V1: &[u8] = b"dclutch:dealer-candidate:v1";
/// PDA domain for persistent Dealer state accounts.
pub const STATE_PDA_DOMAIN_V1: &[u8] = b"dclutch:dealer-state:v1";
/// Digest domain for one complete caller-owned Dealer effect packet.
pub const DEALER_EFFECT_PACKET_DOMAIN_V1: &[u8] = b"dclutch:dealer-effect-packet:v1";

const ACTOR: usize = 0;
const POLICY: usize = 1;
const ACTIVE_CANDIDATE: usize = 2;
const PENDING_CANDIDATE: usize = 3;
const PROPOSED_CANDIDATE: usize = 4;
const STATE: usize = 5;
const ACTIVATION_CACHE: usize = 6;
const REGISTRY_PROGRAM: usize = 7;
const TRADING_PROGRAM: usize = 8;
const TRADING_PROGRAMDATA: usize = 9;
const CUSTODY_PROGRAM: usize = 10;
const CUSTODY_PROGRAMDATA: usize = 11;
const CORE_MARKET: usize = 12;
const CORE_PROGRAM: usize = 13;
const CORE_PROGRAMDATA: usize = 14;
const CLOCK_SYSVAR: usize = 15;
const REALM: usize = 16;
const COLLATERAL_MINT: usize = 17;
const TOKEN_PROGRAM: usize = 18;
const CUSTODY_TRANSFER_AUTHORITY: usize = 19;
const DEALER_QUOTE: usize = 20;
const FEE_VAULT: usize = 21;
const LIVENESS_VAULT: usize = 22;

const CLAIMS_PACKET_MAX_BYTES: usize = CLAIMS_PLAN_HEADER_BYTES_V1 + MAX_OUTCOMES * 8;

/// Stable Dealer SBF refusal.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerSbfError {
    /// Instruction bytes did not encode one canonical Dealer request.
    Instruction = 0x7000,
    /// Account count, order, privilege, aliasing, or executable status refused.
    AccountFrame = 0x7001,
    /// Policy, Candidate, or State owner/PDA/width authentication refused.
    AccountIdentity = 0x7002,
    /// Actor signature or action-specific actor identity refused.
    Signature = 0x7003,
    /// Clock account or request time binding refused.
    Clock = 0x7004,
    /// Registry CPI, receipt provenance, role, program, or release join refused.
    Release = 0x7005,
    /// The total Dealer interpreter refused the transition.
    Semantic = 0x7006,
    /// The canonical Claims child action or receipt refused.
    Claims = 0x7007,
    /// A canonical Custody child transfer, receipt, or postcondition refused.
    Custody = 0x7008,
    /// State data could not be borrowed or commit width changed.
    Commit = 0x7009,
}

// Registered refusal band (`docs/decisions/0007-namespaced-refusal-codes.md`).
// The discriminants stay literal so a code seen in a validator log is greppable;
// these assertions are what stops them drifting out of the allocated band.
const _: () = assert!(
    DealerSbfError::Instruction as u32 == dclutch_refusal_registry::DEALER_REFUSAL_BASE,
    "DealerSbfError must start at its registered refusal band base"
);
const _: () = assert!(
    (DealerSbfError::Commit as u32)
        < dclutch_refusal_registry::DEALER_REFUSAL_BASE + dclutch_refusal_registry::BAND_SPAN,
    "DealerSbfError must not run past its registered refusal band"
);

impl From<DealerSbfError> for ProgramError {
    fn from(value: DealerSbfError) -> Self {
        Self::Custom(value as u32)
    }
}

#[derive(Clone, Copy)]
struct RoleReceipts {
    core: AuthenticatedRoleReceiptV1,
    trading: AuthenticatedRoleReceiptV1,
    custody: AuthenticatedRoleReceiptV1,
}

#[derive(Clone, Copy)]
struct CoreIdentityFacts {
    realm_id: [u8; 32],
    phase: CorePhase,
    winner: u32,
}

#[derive(Clone, Copy)]
struct CoreFacts {
    realm_id: [u8; 32],
    realm: RealmV1,
    token_profile: ExactTransferProfileV1,
    phase: CorePhase,
    winner: u32,
}

struct Prepared {
    policy: Policy,
    pre_active_candidate_id: [u8; 32],
    pre_state_revision: u64,
    plan: Plan,
    post_quote_custody: u64,
    post_fee_custody: u64,
    post_liveness_custody: u64,
    parent_request_digest: [u8; 32],
    realm_id: [u8; 32],
    resources: ChildResources,
    frame: SparseFrame,
    normalized_receipt: [u8; RECEIPT_BYTES],
}

struct ClaimsPacket {
    bytes: [u8; CLAIMS_PACKET_MAX_BYTES],
    len: usize,
    digest: [u8; 32],
}

#[derive(Clone, Copy)]
struct SparseFrame {
    claims_program: Option<usize>,
    claims_programdata: Option<usize>,
    claims_authority: Option<usize>,
    claims_market: Option<usize>,
    dealer_position: Option<usize>,
    actor_position: Option<usize>,
    custody_replay: Option<usize>,
    custody_authorities: [Option<usize>; 3],
    custody_roles: [Option<usize>; 9],
}

impl SparseFrame {
    fn parse(account_count: usize, resources: ChildResources) -> Result<Self, ProgramError> {
        let mut cursor = COMMON_ACCOUNT_COUNT_V1;
        let mut frame = Self {
            claims_program: None,
            claims_programdata: None,
            claims_authority: None,
            claims_market: None,
            dealer_position: None,
            actor_position: None,
            custody_replay: None,
            custody_authorities: [None; 3],
            custody_roles: [None; 9],
        };
        if resources.claims {
            frame.claims_program = Some(take_index(&mut cursor)?);
            frame.claims_programdata = Some(take_index(&mut cursor)?);
            frame.claims_authority = Some(take_index(&mut cursor)?);
            frame.claims_market = Some(take_index(&mut cursor)?);
            if resources.dealer_position {
                frame.dealer_position = Some(take_index(&mut cursor)?);
            }
            if resources.actor_position {
                frame.actor_position = Some(take_index(&mut cursor)?);
            }
        }
        if resources.requires_custody() {
            frame.custody_replay = Some(take_index(&mut cursor)?);
            for slot in frame
                .custody_authorities
                .iter_mut()
                .take(resources.custody_transfer_count)
            {
                *slot = Some(take_index(&mut cursor)?);
            }
            for role in DYNAMIC_CUSTODY_ROLES {
                if resources.requires_custody_role(role) {
                    frame.custody_roles[custody_role_slot(role)] = Some(take_index(&mut cursor)?);
                }
            }
        }
        if cursor != account_count {
            return Err(DealerSbfError::AccountFrame.into());
        }
        Ok(frame)
    }

    fn claims_program(self) -> Result<usize, ProgramError> {
        required_index(self.claims_program)
    }

    fn claims_programdata(self) -> Result<usize, ProgramError> {
        required_index(self.claims_programdata)
    }

    fn claims_authority(self) -> Result<usize, ProgramError> {
        required_index(self.claims_authority)
    }

    fn claims_market(self) -> Result<usize, ProgramError> {
        required_index(self.claims_market)
    }

    fn dealer_position(self) -> Result<usize, ProgramError> {
        required_index(self.dealer_position)
    }

    fn actor_position(self) -> Result<usize, ProgramError> {
        required_index(self.actor_position)
    }

    fn custody_replay(self) -> Result<usize, ProgramError> {
        required_index(self.custody_replay)
    }

    fn custody_authority(self, transfer: usize) -> Result<usize, ProgramError> {
        self.custody_authorities
            .get(transfer)
            .copied()
            .flatten()
            .ok_or_else(|| DealerSbfError::AccountFrame.into())
    }

    const fn custody_role(self, role: CustodyRole) -> Option<usize> {
        self.custody_roles[custody_role_slot(role)]
    }

    fn token_account(self, role: CustodyRole) -> Result<usize, ProgramError> {
        match role {
            CustodyRole::DealerQuote => Ok(DEALER_QUOTE),
            CustodyRole::FeeVault => Ok(FEE_VAULT),
            CustodyRole::LivenessVault => Ok(LIVENESS_VAULT),
            CustodyRole::TakerQuote
            | CustodyRole::Executor
            | CustodyRole::DealerOwner
            | CustodyRole::UnwindRecipient
            | CustodyRole::FeeRecipient
            | CustodyRole::MarketHoard => required_index(self.custody_role(role)),
        }
    }
}

fn take_index(cursor: &mut usize) -> Result<usize, ProgramError> {
    let index = *cursor;
    *cursor = cursor.checked_add(1).ok_or(DealerSbfError::AccountFrame)?;
    Ok(index)
}

fn required_index(index: Option<usize>) -> Result<usize, ProgramError> {
    index.ok_or_else(|| DealerSbfError::AccountFrame.into())
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint_no_alloc!(process_instruction);

/// Execute the standalone Dealer measurement entrypoint.
///
/// Accepted releases use one canonical Trading controller and dispatch to
/// [`process_dealer_family_instruction`]. This wrapper deliberately contains
/// no distinct authentication or economic path.
#[inline(never)]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    process_dealer_family_instruction(program_id, accounts, instruction_data)
}

/// Execute one exact Dealer-family transition under the current Trading program.
///
/// `program_id` is the canonical Trading controller that owns Dealer policy,
/// Candidate, State, and caller-authority PDAs. Callers must pass that same
/// executable program at the fixed Trading-program account. This keeps release
/// authentication and PDA ownership identical whether the handler is exercised
/// through the standalone measurement wrapper or dispatched by the controller.
#[inline(never)]
pub fn process_dealer_family_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let prepared = authenticate_and_prepare(program_id, accounts, instruction_data)?;
    execute_children(program_id, accounts, &prepared)?;
    commit_reinterpreted_state(accounts, &prepared, instruction_data)
}

#[inline(never)]
fn authenticate_and_prepare(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<Prepared, ProgramError> {
    if instruction_data.len() != REQUEST_BYTES {
        return Err(DealerSbfError::Instruction.into());
    }
    let request = Request::decode(instruction_data).map_err(|_| DealerSbfError::Instruction)?;
    validate_prefix(program_id, accounts, request)?;
    let policy = read_policy(program_id, accounts)?;
    authenticate_named_accounts(program_id, accounts, policy)?;
    authenticate_actor(accounts, request)?;
    authenticate_clock(accounts, request)?;
    let receipts = authenticate_roles(accounts, policy.release_set_id)?;
    authenticate_receipt_join(program_id, accounts, policy, receipts)?;
    let core = authenticate_core_market(accounts, policy, request, receipts)?;
    authenticate_token_accounts(accounts, policy, core)?;
    let prepared = prepare(accounts, policy, request, receipts.trading, core.realm_id)?;
    authenticate_sparse_frame(accounts, &prepared, core)?;
    Ok(prepared)
}

#[inline(never)]
fn validate_prefix(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: Request,
) -> ProgramResult {
    if accounts.len() < COMMON_ACCOUNT_COUNT_V1
        || accounts[TRADING_PROGRAM].key != program_id
        || !accounts[ACTOR].is_signer
        || accounts[ACTOR].executable
        || accounts[POLICY].is_signer
        || accounts[POLICY].is_writable
        || accounts[POLICY].executable
        || accounts[ACTIVE_CANDIDATE].is_signer
        || accounts[ACTIVE_CANDIDATE].is_writable
        || accounts[ACTIVE_CANDIDATE].executable
        || accounts[STATE].is_signer
        || !accounts[STATE].is_writable
        || accounts[STATE].executable
        || accounts[ACTIVATION_CACHE].is_signer
        || accounts[ACTIVATION_CACHE].is_writable
        || accounts[ACTIVATION_CACHE].executable
        || !accounts[REGISTRY_PROGRAM].executable
        || accounts[REGISTRY_PROGRAM].is_signer
        || accounts[REGISTRY_PROGRAM].is_writable
        || !accounts[TRADING_PROGRAM].executable
        || accounts[TRADING_PROGRAM].is_signer
        || accounts[TRADING_PROGRAM].is_writable
        || !accounts[CUSTODY_PROGRAM].executable
        || accounts[CUSTODY_PROGRAM].is_signer
        || accounts[CUSTODY_PROGRAM].is_writable
        || !accounts[CORE_PROGRAM].executable
        || accounts[CORE_PROGRAM].is_signer
        || accounts[CORE_PROGRAM].is_writable
        || accounts[CLOCK_SYSVAR].key != &sysvar::clock::ID
        || accounts[CLOCK_SYSVAR].is_signer
        || accounts[CLOCK_SYSVAR].is_writable
        || accounts[CLOCK_SYSVAR].executable
        || accounts[REALM].is_signer
        || accounts[REALM].is_writable
        || accounts[REALM].executable
        || accounts[CORE_MARKET].is_signer
        || accounts[CORE_MARKET].is_writable
        || accounts[CORE_MARKET].executable
        || accounts[COLLATERAL_MINT].is_signer
        || accounts[COLLATERAL_MINT].is_writable
        || accounts[COLLATERAL_MINT].executable
        || accounts[TOKEN_PROGRAM].is_signer
        || accounts[TOKEN_PROGRAM].is_writable
        || !accounts[TOKEN_PROGRAM].executable
    {
        return Err(DealerSbfError::AccountFrame.into());
    }
    for index in [CUSTODY_TRANSFER_AUTHORITY] {
        if accounts[index].is_signer || accounts[index].is_writable || accounts[index].executable {
            return Err(DealerSbfError::AccountFrame.into());
        }
    }
    for index in [TRADING_PROGRAMDATA, CUSTODY_PROGRAMDATA, CORE_PROGRAMDATA] {
        if accounts[index].is_signer || accounts[index].is_writable || accounts[index].executable {
            return Err(DealerSbfError::AccountFrame.into());
        }
    }
    for index in [DEALER_QUOTE, FEE_VAULT, LIVENESS_VAULT] {
        if accounts[index].is_signer || accounts[index].executable {
            return Err(DealerSbfError::AccountFrame.into());
        }
    }
    validate_optional_candidate(accounts, PENDING_CANDIDATE, request.action)?;
    validate_optional_candidate(accounts, PROPOSED_CANDIDATE, request.action)?;
    Ok(())
}

fn validate_optional_candidate(
    accounts: &[AccountInfo<'_>],
    index: usize,
    action: Action,
) -> ProgramResult {
    let account = &accounts[index];
    let absent = account.key == accounts[TRADING_PROGRAM].key;
    if absent {
        if !account.executable || account.is_signer || account.is_writable {
            return Err(DealerSbfError::AccountFrame.into());
        }
        return Ok(());
    }
    if account.executable || account.is_signer || account.is_writable {
        return Err(DealerSbfError::AccountFrame.into());
    }
    if index == PROPOSED_CANDIDATE && action != Action::ScheduleReplacement {
        return Err(DealerSbfError::AccountFrame.into());
    }
    Ok(())
}

#[inline(never)]
fn read_policy(program_id: &Pubkey, accounts: &[AccountInfo<'_>]) -> Result<Policy, ProgramError> {
    let account = &accounts[POLICY];
    if account.owner != program_id || account.data_len() != POLICY_BYTES {
        return Err(DealerSbfError::AccountIdentity.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| DealerSbfError::AccountIdentity)?;
    let policy = Policy::decode(&data).map_err(|_| DealerSbfError::AccountIdentity)?;
    let expected = Pubkey::find_program_address(
        &[POLICY_PDA_DOMAIN_V1, policy.market_id.as_slice()],
        program_id,
    )
    .0;
    if account.key != &expected {
        return Err(DealerSbfError::AccountIdentity.into());
    }
    Ok(policy)
}

#[inline(never)]
fn authenticate_named_accounts(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    policy: Policy,
) -> ProgramResult {
    authenticate_candidate(program_id, &accounts[ACTIVE_CANDIDATE])?;
    if accounts[PENDING_CANDIDATE].key != accounts[TRADING_PROGRAM].key {
        authenticate_candidate(program_id, &accounts[PENDING_CANDIDATE])?;
    }
    if accounts[PROPOSED_CANDIDATE].key != accounts[TRADING_PROGRAM].key {
        authenticate_candidate(program_id, &accounts[PROPOSED_CANDIDATE])?;
    }
    let state = &accounts[STATE];
    if state.owner != program_id
        || state.data_len() != STATE_BYTES
        || state.key
            != &Pubkey::find_program_address(
                &[STATE_PDA_DOMAIN_V1, policy.market_id.as_slice()],
                program_id,
            )
            .0
    {
        return Err(DealerSbfError::AccountIdentity.into());
    }
    Ok(())
}

fn authenticate_candidate(program_id: &Pubkey, account: &AccountInfo<'_>) -> ProgramResult {
    if account.owner != program_id || account.data_len() != CANDIDATE_BYTES {
        return Err(DealerSbfError::AccountIdentity.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| DealerSbfError::AccountIdentity)?;
    let candidate = CandidateView::decode(&data).map_err(|_| DealerSbfError::AccountIdentity)?;
    let expected = Pubkey::find_program_address(
        &[CANDIDATE_PDA_DOMAIN_V1, candidate.candidate_id.as_slice()],
        program_id,
    )
    .0;
    if account.key != &expected {
        return Err(DealerSbfError::AccountIdentity.into());
    }
    Ok(())
}

fn authenticate_actor(accounts: &[AccountInfo<'_>], request: Request) -> ProgramResult {
    let actor = accounts[ACTOR].key.to_bytes();
    match request.action {
        Action::ScheduleReplacement | Action::AddLiquidity | Action::RemoveLiquidity
            if request.actor_id == actor =>
        {
            Ok(())
        }
        Action::ScheduleReplacement | Action::AddLiquidity | Action::RemoveLiquidity => {
            Err(DealerSbfError::Signature.into())
        }
        Action::EnterTerminal => Ok(()),
        _ if request.actor_id == [0; 32] => Ok(()),
        _ => Err(DealerSbfError::Signature.into()),
    }
}

#[inline(never)]
fn authenticate_claims_accounts(
    accounts: &[AccountInfo<'_>],
    policy: Policy,
    core: CoreFacts,
    frame: SparseFrame,
) -> ProgramResult {
    let claims_program = frame.claims_program()?;
    let claims_programdata = frame.claims_programdata()?;
    let claims = accounts[claims_program].key;
    let receipt = authenticate_activated_role(
        accounts,
        policy.release_set_id,
        ExecutionRoleV1::Claims,
        claims_program,
        claims_programdata,
    )?;
    if receipt.program().as_bytes() != &claims.to_bytes() {
        return Err(DealerSbfError::Release.into());
    }
    let market_index = frame.claims_market()?;
    let market = &accounts[market_index];
    if market.owner != claims {
        return Err(DealerSbfError::AccountIdentity.into());
    }
    let aggregate_seeds = ClaimsAggregateSeedsV1::new(policy.market_id)
        .map_err(|_| DealerSbfError::AccountIdentity)?;
    if market.key != &Pubkey::find_program_address(&aggregate_seeds.as_slices(), claims).0 {
        return Err(DealerSbfError::AccountIdentity.into());
    }
    let market_data = market
        .try_borrow_data()
        .map_err(|_| DealerSbfError::AccountIdentity)?;
    let outcomes =
        market_outcome_count(&market_data).map_err(|_| DealerSbfError::AccountIdentity)?;
    if outcomes != u32::from(policy.outcome_count)
        || market_identity(&market_data).map_err(|_| DealerSbfError::AccountIdentity)?
            != policy.market_id
        || market_release_set_id(&market_data).map_err(|_| DealerSbfError::AccountIdentity)?
            != policy.release_set_id
        || market_registry_program(&market_data).map_err(|_| DealerSbfError::AccountIdentity)?
            != accounts[REGISTRY_PROGRAM].key.to_bytes()
        || !claims_phase_matches_core(
            market_phase(&market_data).map_err(|_| DealerSbfError::AccountIdentity)?,
            core.phase,
            core.winner,
        )
    {
        return Err(DealerSbfError::AccountIdentity.into());
    }
    drop(market_data);
    if frame.dealer_position.is_some() {
        let dealer_position = frame.dealer_position()?;
        authenticate_position(
            &accounts[dealer_position],
            claims,
            policy.market_id,
            policy.dealer_id,
            outcomes,
        )?;
    }
    if frame.actor_position.is_some() {
        let actor_position = frame.actor_position()?;
        authenticate_position(
            &accounts[actor_position],
            claims,
            policy.market_id,
            accounts[ACTOR].key.to_bytes(),
            outcomes,
        )?;
    }
    Ok(())
}

#[inline(never)]
fn authenticate_core_market(
    accounts: &[AccountInfo<'_>],
    policy: Policy,
    request: Request,
    receipts: RoleReceipts,
) -> Result<CoreFacts, ProgramError> {
    let core = authenticate_core_identity(accounts, policy, request)?;
    if receipts.core.execution_release_set_id().as_bytes() != &policy.release_set_id {
        return Err(DealerSbfError::Release.into());
    }
    let (realm, token_profile) = authenticate_core_realm(accounts, core.realm_id)?;
    Ok(CoreFacts {
        realm_id: core.realm_id,
        realm,
        token_profile,
        phase: core.phase,
        winner: core.winner,
    })
}

#[inline(never)]
fn authenticate_core_identity(
    accounts: &[AccountInfo<'_>],
    policy: Policy,
    request: Request,
) -> Result<CoreIdentityFacts, ProgramError> {
    let account = &accounts[CORE_MARKET];
    if account.owner != accounts[CORE_PROGRAM].key || account.key.to_bytes() != policy.market_id {
        return Err(DealerSbfError::AccountIdentity.into());
    }
    let core = decode_core(account)?;
    let expected = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(core.identity).as_slices(),
        accounts[CORE_PROGRAM].key,
    )
    .0;
    if account.key != &expected
        || core.identity.market_id.to_bytes() != policy.market_id
        || core.identity.selected_release_set.to_bytes() != policy.release_set_id
        || !core_phase_admits(request, core.phase, core.terminal_winner, policy.market_id)
    {
        return Err(DealerSbfError::AccountIdentity.into());
    }
    Ok(CoreIdentityFacts {
        realm_id: core.identity.realm_id.to_bytes(),
        phase: core.phase,
        winner: core.terminal_winner,
    })
}

#[inline(never)]
fn authenticate_core_realm(
    accounts: &[AccountInfo<'_>],
    realm_id: [u8; 32],
) -> Result<(RealmV1, ExactTransferProfileV1), ProgramError> {
    if accounts[REALM].owner != accounts[CORE_PROGRAM].key
        || accounts[REALM].key
            != &Pubkey::find_program_address(
                &[REALM_PDA_DOMAIN, realm_id.as_slice()],
                accounts[CORE_PROGRAM].key,
            )
            .0
    {
        return Err(DealerSbfError::AccountIdentity.into());
    }
    let realm_data = accounts[REALM]
        .try_borrow_data()
        .map_err(|_| DealerSbfError::AccountIdentity)?;
    let observed_realm_id = hash(&realm_data).to_bytes();
    let realm = RealmV1::decode(&realm_data).map_err(|_| DealerSbfError::AccountIdentity)?;
    if observed_realm_id != realm_id
        || realm.collateral_mint() != &accounts[COLLATERAL_MINT].key.to_bytes()
        || realm.token_program() != &accounts[TOKEN_PROGRAM].key.to_bytes()
    {
        return Err(DealerSbfError::AccountIdentity.into());
    }
    let token_profile = PRODUCTION_ADAPTER_RELEASES
        .iter()
        .find(|release| hash(&release.to_bytes()).as_ref() == realm.collateral_adapter_release_id())
        .map(|release| release.profile())
        .ok_or(DealerSbfError::AccountIdentity)?;
    Ok((realm, token_profile))
}

#[inline(never)]
fn decode_core(account: &AccountInfo<'_>) -> Result<CoreState, ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| DealerSbfError::AccountIdentity)?;
    CoreState::decode(&data).map_err(|_| DealerSbfError::AccountIdentity.into())
}

fn core_phase_admits(request: Request, phase: CorePhase, winner: u32, market: [u8; 32]) -> bool {
    match request.action {
        Action::ScheduleReplacement
        | Action::ActivateReplacement
        | Action::Fill
        | Action::AddLiquidity
        | Action::RemoveLiquidity => {
            matches!(phase, CorePhase::Open)
        }
        Action::EnterTerminal => {
            (matches!(phase, CorePhase::Terminal | CorePhase::Retiring))
                && request.actor_id == market
                && u32::from(request.outcome) == winner
        }
        Action::Unwind | Action::Retire => matches!(phase, CorePhase::Retiring),
    }
}

fn claims_phase_matches_core(claims: ClaimsPhase, core: CorePhase, winner: u32) -> bool {
    match (claims, core) {
        (ClaimsPhase::Open, CorePhase::Open) | (ClaimsPhase::Retired, CorePhase::Retired) => true,
        (ClaimsPhase::Terminal(claims_winner), CorePhase::Terminal)
        | (ClaimsPhase::Retiring(claims_winner), CorePhase::Retiring) => claims_winner == winner,
        _ => false,
    }
}

fn authenticate_position(
    account: &AccountInfo<'_>,
    claims_program: &Pubkey,
    market: [u8; 32],
    owner: [u8; 32],
    outcomes: u32,
) -> ProgramResult {
    if account.owner != claims_program {
        return Err(DealerSbfError::AccountIdentity.into());
    }
    let position_seeds =
        ClaimsPositionSeedsV1::new(market, owner).map_err(|_| DealerSbfError::AccountIdentity)?;
    if account.key != &Pubkey::find_program_address(&position_seeds.as_slices(), claims_program).0 {
        return Err(DealerSbfError::AccountIdentity.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| DealerSbfError::AccountIdentity)?;
    if position_owner(&data, outcomes).map_err(|_| DealerSbfError::AccountIdentity)? != owner
        || position_market_id(&data, outcomes).map_err(|_| DealerSbfError::AccountIdentity)?
            != market
    {
        return Err(DealerSbfError::AccountIdentity.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_sparse_frame(
    accounts: &[AccountInfo<'_>],
    prepared: &Prepared,
    core: CoreFacts,
) -> ProgramResult {
    validate_sparse_privileges(accounts, prepared.frame, prepared.resources)?;
    if prepared.resources.claims {
        authenticate_claims_accounts(accounts, prepared.policy, core, prepared.frame)?;
    }
    if prepared.resources.requires_custody() {
        authenticate_dynamic_custody_accounts(accounts, prepared, core)?;
    }
    Ok(())
}

fn validate_sparse_privileges(
    accounts: &[AccountInfo<'_>],
    frame: SparseFrame,
    resources: ChildResources,
) -> ProgramResult {
    for (role, index) in [
        (CustodyRole::DealerQuote, DEALER_QUOTE),
        (CustodyRole::FeeVault, FEE_VAULT),
        (CustodyRole::LivenessVault, LIVENESS_VAULT),
    ] {
        if accounts[index].is_writable != resources.requires_custody_role(role) {
            return Err(DealerSbfError::AccountFrame.into());
        }
    }
    if resources.claims {
        let claims_program = frame.claims_program()?;
        let claims_programdata = frame.claims_programdata()?;
        let claims_authority = frame.claims_authority()?;
        let claims_market = frame.claims_market()?;
        if !accounts[claims_program].executable
            || accounts[claims_program].is_signer
            || accounts[claims_program].is_writable
            || accounts[claims_programdata].executable
            || accounts[claims_programdata].is_signer
            || accounts[claims_programdata].is_writable
            || accounts[claims_authority].executable
            || accounts[claims_authority].is_signer
            || accounts[claims_authority].is_writable
            || accounts[claims_market].executable
            || accounts[claims_market].is_signer
            || !accounts[claims_market].is_writable
        {
            return Err(DealerSbfError::AccountFrame.into());
        }
        for index in [frame.dealer_position, frame.actor_position]
            .into_iter()
            .flatten()
        {
            if accounts[index].executable
                || accounts[index].is_signer
                || !accounts[index].is_writable
            {
                return Err(DealerSbfError::AccountFrame.into());
            }
        }
    }
    if resources.requires_custody() {
        let replay = frame.custody_replay()?;
        if accounts[replay].executable
            || accounts[replay].is_signer
            || !accounts[replay].is_writable
        {
            return Err(DealerSbfError::AccountFrame.into());
        }
        for authority in frame.custody_authorities.into_iter().flatten() {
            if accounts[authority].executable
                || accounts[authority].is_signer
                || accounts[authority].is_writable
            {
                return Err(DealerSbfError::AccountFrame.into());
            }
        }
        for role in DYNAMIC_CUSTODY_ROLES {
            if let Some(index) = frame.custody_role(role)
                && (accounts[index].executable
                    || accounts[index].is_signer
                    || !accounts[index].is_writable)
            {
                return Err(DealerSbfError::AccountFrame.into());
            }
        }
    }
    Ok(())
}

#[inline(never)]
fn authenticate_dynamic_custody_accounts(
    accounts: &[AccountInfo<'_>],
    prepared: &Prepared,
    core: CoreFacts,
) -> ProgramResult {
    let custody_authority = *accounts[CUSTODY_TRANSFER_AUTHORITY].key;
    for role in DYNAMIC_CUSTODY_ROLES {
        let Some(index) = prepared.frame.custody_role(role) else {
            continue;
        };
        match role {
            CustodyRole::MarketHoard => {
                authenticate_vault(
                    accounts,
                    prepared.policy,
                    core.token_profile,
                    index,
                    prepared.policy.market_id,
                    CompartmentV1::HoardPrincipal,
                    custody_authority,
                )?;
            }
            CustodyRole::TakerQuote
            | CustodyRole::Executor
            | CustodyRole::DealerOwner
            | CustodyRole::UnwindRecipient
            | CustodyRole::FeeRecipient => {
                authenticate_external_token_account(
                    accounts,
                    prepared,
                    core.token_profile,
                    index,
                    role,
                )?;
            }
            CustodyRole::DealerQuote | CustodyRole::FeeVault | CustodyRole::LivenessVault => {
                return Err(DealerSbfError::AccountFrame.into());
            }
        }
    }
    for transfer in prepared.plan.custody.into_iter().flatten() {
        let source = prepared.frame.token_account(transfer.source)?;
        let destination = prepared.frame.token_account(transfer.destination)?;
        if source == destination || accounts[source].key == accounts[destination].key {
            return Err(DealerSbfError::AccountFrame.into());
        }
    }
    Ok(())
}

fn authenticate_external_token_account(
    accounts: &[AccountInfo<'_>],
    prepared: &Prepared,
    profile: ExactTransferProfileV1,
    index: usize,
    role: CustodyRole,
) -> ProgramResult {
    let account = &accounts[index];
    if account.owner != accounts[TOKEN_PROGRAM].key {
        return Err(DealerSbfError::AccountIdentity.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| DealerSbfError::AccountIdentity)?;
    let token = profile
        .check_transfer_account(accounts[TOKEN_PROGRAM].key.to_bytes(), &data)
        .map_err(|_| DealerSbfError::AccountIdentity)?;
    let expected_owner =
        external_owner(accounts, prepared.policy, role).ok_or(DealerSbfError::AccountIdentity)?;
    if token.mint != accounts[COLLATERAL_MINT].key.to_bytes() || token.owner != expected_owner {
        return Err(DealerSbfError::AccountIdentity.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_token_accounts(
    accounts: &[AccountInfo<'_>],
    policy: Policy,
    core: CoreFacts,
) -> ProgramResult {
    let token_program = accounts[TOKEN_PROGRAM].key.to_bytes();
    core.token_profile
        .check_program(token_program)
        .map_err(|_| DealerSbfError::AccountIdentity)?;
    if accounts[COLLATERAL_MINT].owner != accounts[TOKEN_PROGRAM].key
        || accounts[COLLATERAL_MINT].key.to_bytes() != *core.realm.collateral_mint()
    {
        return Err(DealerSbfError::AccountIdentity.into());
    }
    let mint_data = accounts[COLLATERAL_MINT]
        .try_borrow_data()
        .map_err(|_| DealerSbfError::AccountIdentity)?;
    let mint = core
        .token_profile
        .check_mint(token_program, &mint_data)
        .map_err(|_| DealerSbfError::AccountIdentity)?;
    if (core.realm.mint_authority_policy() == MintAuthorityPolicy::RequireAbsent
        && !matches!(mint.mint_authority, COption::None))
        || (core.realm.freeze_authority_policy() == FreezeAuthorityPolicy::RequireAbsent
            && !matches!(mint.freeze_authority, COption::None))
    {
        return Err(DealerSbfError::AccountIdentity.into());
    }
    drop(mint_data);
    let expected_custody_authority = Pubkey::find_program_address(
        &[
            CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
            policy.market_id.as_slice(),
            policy.release_set_id.as_slice(),
        ],
        accounts[CUSTODY_PROGRAM].key,
    )
    .0;
    if accounts[CUSTODY_TRANSFER_AUTHORITY].key != &expected_custody_authority {
        return Err(DealerSbfError::AccountIdentity.into());
    }
    let state_context = accounts[STATE].key.to_bytes();
    let dealer_quote = authenticate_vault(
        accounts,
        policy,
        core.token_profile,
        DEALER_QUOTE,
        state_context,
        CompartmentV1::TradingPrincipal,
        expected_custody_authority,
    )?;
    let fee_vault = authenticate_vault(
        accounts,
        policy,
        core.token_profile,
        FEE_VAULT,
        state_context,
        CompartmentV1::FeeVault,
        expected_custody_authority,
    )?;
    let liveness_vault = authenticate_vault(
        accounts,
        policy,
        core.token_profile,
        LIVENESS_VAULT,
        state_context,
        CompartmentV1::LivenessVault,
        expected_custody_authority,
    )?;
    let state_data = accounts[STATE]
        .try_borrow_data()
        .map_err(|_| DealerSbfError::AccountIdentity)?;
    let state = State::decode(&state_data).map_err(|_| DealerSbfError::AccountIdentity)?;
    if dealer_quote.amount != state.quote_custody
        || fee_vault.amount != state.fee_custody
        || liveness_vault.amount != state.liveness_custody
    {
        return Err(DealerSbfError::AccountIdentity.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_vault(
    accounts: &[AccountInfo<'_>],
    policy: Policy,
    profile: ExactTransferProfileV1,
    index: usize,
    context: [u8; 32],
    compartment: CompartmentV1,
    custody_authority: Pubkey,
) -> Result<TokenAccount, ProgramError> {
    let seeds = CustodyVaultSeedsV1::new(
        policy.market_id,
        policy.release_set_id,
        context,
        compartment,
    );
    let account = &accounts[index];
    let expected =
        Pubkey::find_program_address(&seeds.as_slices(), accounts[CUSTODY_PROGRAM].key).0;
    if account.key != &expected || account.owner != accounts[TOKEN_PROGRAM].key {
        return Err(DealerSbfError::AccountIdentity.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| DealerSbfError::AccountIdentity)?;
    profile
        .check_custody_account(
            accounts[TOKEN_PROGRAM].key.to_bytes(),
            &data,
            accounts[COLLATERAL_MINT].key.to_bytes(),
            custody_authority.to_bytes(),
        )
        .map_err(|_| DealerSbfError::AccountIdentity.into())
}

fn read_token(accounts: &[AccountInfo<'_>], index: usize) -> Result<TokenAccount, ProgramError> {
    let account = &accounts[index];
    if account.owner != accounts[TOKEN_PROGRAM].key {
        return Err(DealerSbfError::AccountIdentity.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| DealerSbfError::AccountIdentity)?;
    let token = TokenAccount::parse(&data).map_err(|_| DealerSbfError::AccountIdentity)?;
    if token.mint != accounts[COLLATERAL_MINT].key.to_bytes() {
        return Err(DealerSbfError::AccountIdentity.into());
    }
    Ok(token)
}

/// Bind the request's `now` coordinate to the executing slot.
///
/// The expectation is DERIVED from [`Action::now_discipline`], the codec's
/// single statement of which commands carry a slot in the Dealer semantics.
/// Restating it here as a hand-listed exemption is what made `AddLiquidity`
/// and `RemoveLiquidity` unreachable at every slot the chain can offer: their
/// request shape requires `now == 0` and this check named only `Retire`, so no
/// encoding satisfied both. A slot expectation may only ever be `0` or
/// `clock.slot`; nothing else is representable.
fn authenticate_clock(accounts: &[AccountInfo<'_>], request: Request) -> ProgramResult {
    let clock =
        Clock::from_account_info(&accounts[CLOCK_SYSVAR]).map_err(|_| DealerSbfError::Clock)?;
    let expected = match request.action.now_discipline() {
        NowDisciplineV1::CanonicalZero => 0,
        NowDisciplineV1::ExecutionSlot => clock.slot,
    };
    if request.now != expected {
        return Err(DealerSbfError::Clock.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_roles(
    accounts: &[AccountInfo<'_>],
    release_set_id: [u8; 32],
) -> Result<RoleReceipts, ProgramError> {
    Ok(RoleReceipts {
        core: authenticate_activated_role(
            accounts,
            release_set_id,
            ExecutionRoleV1::Core,
            CORE_PROGRAM,
            CORE_PROGRAMDATA,
        )?,
        trading: authenticate_trading_controller_release(accounts, release_set_id)?,
        custody: authenticate_activated_role(
            accounts,
            release_set_id,
            ExecutionRoleV1::Custody,
            CUSTODY_PROGRAM,
            CUSTODY_PROGRAMDATA,
        )?,
    })
}

/// Isolated release-authentication seam for the single Trading controller.
///
/// Dealer family logic never selects or authenticates a family-specific
/// program. The canonical controller is the sole Registry Trading deployment,
/// and both standalone measurement and controller dispatch use this exact join.
#[inline(never)]
fn authenticate_trading_controller_release(
    accounts: &[AccountInfo<'_>],
    release_set_id: [u8; 32],
) -> Result<AuthenticatedRoleReceiptV1, ProgramError> {
    authenticate_activated_role(
        accounts,
        release_set_id,
        ExecutionRoleV1::Trading,
        TRADING_PROGRAM,
        TRADING_PROGRAMDATA,
    )
}

/// Authenticate one activated role directly out of the Registry-owned cache.
///
/// Dealer is reached as a child of a Registry continuation, where the Registry
/// already sits at CPI depth one, so the `RegistryInstructionV1::Reauthenticate`
/// invocation this replaced was reentrancy and Solana refused it before Dealer
/// could do any work. The retired route has no fallback: every fact it returned
/// is in the Registry-owned cache the frame already carries, and
/// `dclutch-registry-activation-auth-v1` is the Registry's own code for reading
/// it.
#[inline(never)]
fn authenticate_activated_role(
    accounts: &[AccountInfo<'_>],
    release_set_id: [u8; 32],
    role: ExecutionRoleV1,
    program_index: usize,
    programdata_index: usize,
) -> Result<AuthenticatedRoleReceiptV1, ProgramError> {
    let registry = accounts
        .get(REGISTRY_PROGRAM)
        .ok_or(DealerSbfError::AccountFrame)?;
    let cache = accounts
        .get(ACTIVATION_CACHE)
        .ok_or(DealerSbfError::AccountFrame)?;
    let program = accounts
        .get(program_index)
        .ok_or(DealerSbfError::AccountFrame)?;
    let programdata = accounts
        .get(programdata_index)
        .ok_or(DealerSbfError::AccountFrame)?;
    authenticate_activated_role_v1(registry, cache, &release_set_id, role, program, programdata)
        .map_err(|_| DealerSbfError::Release.into())
}

fn authenticate_receipt_join(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    policy: Policy,
    receipts: RoleReceipts,
) -> ProgramResult {
    let selected = policy.release_set_id;
    if receipts.trading.execution_release_set_id().as_bytes() != &selected
        || receipts.core.execution_release_set_id().as_bytes() != &selected
        || receipts.custody.execution_release_set_id().as_bytes() != &selected
        || receipts.core.program().as_bytes() != &accounts[CORE_PROGRAM].key.to_bytes()
        || receipts.trading.program().as_bytes() != &program_id.to_bytes()
        || receipts.custody.program().as_bytes() != &accounts[CUSTODY_PROGRAM].key.to_bytes()
    {
        return Err(DealerSbfError::Release.into());
    }
    Ok(())
}

#[inline(never)]
fn prepare(
    accounts: &[AccountInfo<'_>],
    policy: Policy,
    request: Request,
    trading_receipt: AuthenticatedRoleReceiptV1,
    realm_id: [u8; 32],
) -> Result<Prepared, ProgramError> {
    let normalized = ReleaseReceipt {
        registry_program: accounts[REGISTRY_PROGRAM].key.to_bytes(),
        release_set_id: *trading_receipt.execution_release_set_id().as_bytes(),
        program: *trading_receipt.program().as_bytes(),
        artifact_release: *trading_receipt.artifact_release_id().as_bytes(),
        semantic_release: *trading_receipt.semantic_release_id().as_bytes(),
    }
    .to_bytes()
    .map_err(|_| DealerSbfError::Release)?;
    let policy_data = accounts[POLICY]
        .try_borrow_data()
        .map_err(|_| DealerSbfError::Semantic)?;
    let active_data = accounts[ACTIVE_CANDIDATE]
        .try_borrow_data()
        .map_err(|_| DealerSbfError::Semantic)?;
    let pending_data = if accounts[PENDING_CANDIDATE].key == accounts[TRADING_PROGRAM].key {
        None
    } else {
        Some(
            accounts[PENDING_CANDIDATE]
                .try_borrow_data()
                .map_err(|_| DealerSbfError::Semantic)?,
        )
    };
    let proposed_data = if accounts[PROPOSED_CANDIDATE].key == accounts[TRADING_PROGRAM].key {
        None
    } else {
        Some(
            accounts[PROPOSED_CANDIDATE]
                .try_borrow_data()
                .map_err(|_| DealerSbfError::Semantic)?,
        )
    };
    let state_data = accounts[STATE]
        .try_borrow_data()
        .map_err(|_| DealerSbfError::Semantic)?;
    let (pre_active_candidate_id, pre_state_revision) = state_context(&state_data)?;
    let request_bytes = request
        .to_bytes()
        .map_err(|_| DealerSbfError::Instruction)?;
    let absent = [0_u8; 1];
    let input_digest = hashv(&[
        DEALER_EFFECT_PACKET_DOMAIN_V1,
        accounts[STATE].key.as_ref(),
        &policy_data,
        &active_data,
        pending_data
            .as_ref()
            .map_or(absent.as_slice(), |data| &data[..]),
        proposed_data
            .as_ref()
            .map_or(absent.as_slice(), |data| &data[..]),
        &state_data,
        &request_bytes,
        &normalized,
    ])
    .to_bytes();
    let transition = interpret(Inputs {
        policy: &policy_data,
        active_candidate: &active_data,
        pending_candidate: pending_data.as_ref().map(|data| &data[..]),
        proposed_candidate: proposed_data.as_ref().map(|data| &data[..]),
        release_receipt: &normalized,
        state: &state_data,
        request: &request_bytes,
    })
    .map_err(|_| DealerSbfError::Semantic)?;
    drop(policy_data);
    drop(active_data);
    drop(pending_data);
    drop(proposed_data);
    drop(state_data);
    let parent_request_digest = parent_request_digest(input_digest, transition.post)?;
    let resources =
        ChildResources::derive(transition.plan).map_err(|_| DealerSbfError::AccountFrame)?;
    let frame = SparseFrame::parse(accounts.len(), resources)?;
    Ok(Prepared {
        policy,
        pre_active_candidate_id,
        pre_state_revision,
        plan: transition.plan,
        post_quote_custody: transition.post.quote_custody,
        post_fee_custody: transition.post.fee_custody,
        post_liveness_custody: transition.post.liveness_custody,
        parent_request_digest,
        realm_id,
        resources,
        frame,
        normalized_receipt: normalized,
    })
}

#[inline(never)]
fn parent_request_digest(input_digest: [u8; 32], post: State) -> Result<[u8; 32], ProgramError> {
    let post_bytes = post.to_bytes().map_err(|_| DealerSbfError::Semantic)?;
    Ok(hashv(&[DEALER_EFFECT_PACKET_DOMAIN_V1, &input_digest, &post_bytes]).to_bytes())
}

#[inline(never)]
fn state_context(state: &[u8]) -> Result<([u8; 32], u64), ProgramError> {
    let value = State::decode(state).map_err(|_| DealerSbfError::Semantic)?;
    Ok((value.active_candidate_id, value.state_revision))
}

#[inline(never)]
fn execute_children(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    prepared: &Prepared,
) -> ProgramResult {
    execute_claims_child(program_id, accounts, prepared)?;
    execute_custody_children(program_id, accounts, prepared)
}

#[inline(never)]
fn execute_claims_child(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    prepared: &Prepared,
) -> ProgramResult {
    let claims = prepare_claims_packet(accounts, prepared)?;
    match (prepared.resources.claims, claims) {
        (true, Some(packet)) => {
            let claims_authority = prepared.frame.claims_authority()?;
            authenticate_caller_authority(
                program_id,
                accounts,
                claims_authority,
                prepared,
                prepared.parent_request_digest,
                packet.digest,
            )?;
            invoke_claims(program_id, accounts, prepared, &packet)?;
        }
        (false, None) => {}
        (true, None) | (false, Some(_)) => {
            return Err(DealerSbfError::AccountFrame.into());
        }
    }
    Ok(())
}

#[inline(never)]
fn invoke_claims(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    prepared: &Prepared,
    packet: &ClaimsPacket,
) -> ProgramResult {
    let data = packet
        .bytes
        .get(..packet.len)
        .ok_or(DealerSbfError::Claims)?;
    let plan = ClaimsPlanV1::decode(data).map_err(|_| DealerSbfError::Claims)?;
    let (source, destination) = claims_position_indices(plan, prepared)?;
    let dealer_position = prepared.frame.dealer_position()?;
    let actor_position = prepared.frame.actor_position;
    let uses_dealer = source == dealer_position || destination == dealer_position;
    let uses_actor = actor_position.is_some_and(|index| source == index || destination == index);
    if prepared.resources.dealer_position != uses_dealer
        || prepared.resources.actor_position != uses_actor
    {
        return Err(DealerSbfError::AccountFrame.into());
    }
    let claims_program = prepared.frame.claims_program()?;
    let claims_programdata = prepared.frame.claims_programdata()?;
    let claims_authority = prepared.frame.claims_authority()?;
    let claims_market = prepared.frame.claims_market()?;
    let instruction = Instruction {
        program_id: *accounts[claims_program].key,
        accounts: Vec::from([
            AccountMeta::new_readonly(*accounts[claims_authority].key, true),
            AccountMeta::new(*accounts[claims_market].key, false),
            position_meta(accounts, source, claims_program),
            position_meta(accounts, destination, claims_program),
            AccountMeta::new_readonly(*accounts[ACTIVATION_CACHE].key, false),
            AccountMeta::new_readonly(*accounts[TRADING_PROGRAM].key, false),
            AccountMeta::new_readonly(*accounts[TRADING_PROGRAMDATA].key, false),
            AccountMeta::new_readonly(*accounts[claims_program].key, false),
            AccountMeta::new_readonly(*accounts[claims_programdata].key, false),
            AccountMeta::new_readonly(*accounts[REGISTRY_PROGRAM].key, false),
            AccountMeta::new_readonly(*accounts[CORE_MARKET].key, false),
            AccountMeta::new_readonly(*accounts[CORE_PROGRAM].key, false),
            AccountMeta::new_readonly(*accounts[CORE_PROGRAMDATA].key, false),
        ]),
        data: data.to_vec(),
    };
    let child_accounts = [
        accounts[claims_authority].clone(),
        accounts[claims_market].clone(),
        accounts[source].clone(),
        accounts[destination].clone(),
        accounts[ACTIVATION_CACHE].clone(),
        accounts[TRADING_PROGRAM].clone(),
        accounts[TRADING_PROGRAMDATA].clone(),
        accounts[claims_program].clone(),
        accounts[claims_programdata].clone(),
        accounts[REGISTRY_PROGRAM].clone(),
        accounts[CORE_MARKET].clone(),
        accounts[CORE_PROGRAM].clone(),
        accounts[CORE_PROGRAMDATA].clone(),
    ];
    invoke_as_trading(
        program_id,
        accounts,
        claims_authority,
        prepared,
        prepared.parent_request_digest,
        packet.digest,
        &instruction,
        &child_accounts,
        DealerSbfError::Claims,
    )?;
    verify_claims_receipt(accounts, prepared, plan, packet.digest, source, destination)
}

fn claims_position_indices(
    plan: ClaimsPlanV1<'_>,
    prepared: &Prepared,
) -> Result<(usize, usize), ProgramError> {
    let source = if plan.source_owner() == prepared.policy.dealer_id {
        prepared.frame.dealer_position()?
    } else {
        prepared.frame.actor_position()?
    };
    let destination = if plan.expected_destination_revision() == NO_POSITION_REVISION {
        prepared.frame.claims_program()?
    } else if plan.destination_owner() == prepared.policy.dealer_id {
        prepared.frame.dealer_position()?
    } else {
        prepared.frame.actor_position()?
    };
    if source == destination {
        return Err(DealerSbfError::Claims.into());
    }
    Ok((source, destination))
}

fn position_meta(accounts: &[AccountInfo<'_>], index: usize, claims_program: usize) -> AccountMeta {
    if index == claims_program {
        AccountMeta::new_readonly(*accounts[index].key, false)
    } else {
        AccountMeta::new(*accounts[index].key, false)
    }
}

#[allow(clippy::too_many_arguments)]
fn invoke_as_trading<'info>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'info>],
    authority_index: usize,
    prepared: &Prepared,
    context: [u8; 32],
    role_request_digest: [u8; 32],
    instruction: &Instruction,
    child_accounts: &[AccountInfo<'info>],
    error: DealerSbfError,
) -> ProgramResult {
    let release = ContentId::new(prepared.policy.release_set_id).map_err(|_| error)?;
    let seeds = CallerAuthoritySeedsV1::new(
        release,
        prepared.policy.market_id,
        ExecutionRoleV1::Trading,
        context,
        role_request_digest,
    )
    .map_err(|_| error)?;
    let bump = Pubkey::find_program_address(&seeds.as_slices(), program_id).1;
    let bump_seed = [bump];
    let [domain, release, market, role, context, request] = seeds.as_slices();
    if accounts[authority_index].key
        != &Pubkey::create_program_address(
            &[domain, release, market, role, context, request, &bump_seed],
            program_id,
        )
        .map_err(|_| error)?
    {
        return Err(error.into());
    }
    invoke_signed(
        instruction,
        child_accounts,
        &[&[domain, release, market, role, context, request, &bump_seed]],
    )
    .map_err(|_| error.into())
}

fn verify_claims_receipt(
    accounts: &[AccountInfo<'_>],
    prepared: &Prepared,
    plan: ClaimsPlanV1<'_>,
    packet_digest: [u8; 32],
    source: usize,
    destination: usize,
) -> ProgramResult {
    let (producer, bytes) = get_return_data().ok_or(DealerSbfError::Claims)?;
    let claims_program = prepared.frame.claims_program()?;
    if producer != *accounts[claims_program].key {
        return Err(DealerSbfError::Claims.into());
    }
    let receipt = ClaimsReceiptV1::decode(&bytes).map_err(|_| DealerSbfError::Claims)?;
    let expected_payout = match prepared.plan.claim {
        ClaimAction::Redeem { payout, .. } => payout,
        ClaimAction::None | ClaimAction::Transfer { .. } | ClaimAction::AdjustLiquidity { .. } => 0,
    };
    let post_market = read_market_revision(accounts, prepared.frame.claims_market()?)?;
    let post_source = read_position_revision(accounts, source, plan.outcome_count())?;
    let post_destination = if destination == claims_program {
        NO_POSITION_REVISION
    } else {
        read_position_revision(accounts, destination, plan.outcome_count())?
    };
    if receipt.caller_role() != CallerRole::Trading
        || receipt.action() != plan.action()
        || receipt.release_set_id() != plan.release_set_id()
        || receipt.market() != plan.market()
        || receipt.request_id() != plan.request_id()
        || receipt.packet_digest() != packet_digest
        || receipt.claims_program() != accounts[claims_program].key.to_bytes()
        || receipt.pre_market_revision() != plan.expected_market_revision()
        || receipt.post_market_revision() != post_market
        || receipt.post_source_revision() != post_source
        || receipt.post_destination_revision() != post_destination
        || receipt.payout() != expected_payout
    {
        return Err(DealerSbfError::Claims.into());
    }
    Ok(())
}

fn read_market_revision(
    accounts: &[AccountInfo<'_>],
    claims_market: usize,
) -> Result<u64, ProgramError> {
    let data = accounts[claims_market]
        .try_borrow_data()
        .map_err(|_| DealerSbfError::Claims)?;
    market_revision(&data).map_err(|_| DealerSbfError::Claims.into())
}

fn read_position_revision(
    accounts: &[AccountInfo<'_>],
    index: usize,
    outcomes: u32,
) -> Result<u64, ProgramError> {
    let data = accounts[index]
        .try_borrow_data()
        .map_err(|_| DealerSbfError::Claims)?;
    position_revision(&data, outcomes).map_err(|_| DealerSbfError::Claims.into())
}

#[inline(never)]
fn execute_custody_children(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    prepared: &Prepared,
) -> ProgramResult {
    if !prepared.resources.requires_custody() {
        return verify_final_custody_projection(accounts, prepared);
    }
    let replay_index = prepared.frame.custody_replay()?;
    let replay_data = accounts[replay_index]
        .try_borrow_data()
        .map_err(|_| DealerSbfError::Custody)?;
    let replay = CustodyReplayV1::decode(&replay_data).map_err(|_| DealerSbfError::Custody)?;
    drop(replay_data);
    let mut expected_revision = replay.next_revision;
    let mut observed_transfer_count = 0_usize;
    for (index, transfer) in prepared.plan.custody.iter().enumerate() {
        if let Some(transfer) = transfer {
            if !prepared.resources.requires_custody_role(transfer.source)
                || !prepared
                    .resources
                    .requires_custody_role(transfer.destination)
            {
                return Err(DealerSbfError::AccountFrame.into());
            }
            execute_one_custody_child(
                program_id,
                accounts,
                prepared,
                *transfer,
                index,
                expected_revision,
                replay.generation,
            )?;
            expected_revision = expected_revision
                .checked_add(1)
                .ok_or(DealerSbfError::Custody)?;
            observed_transfer_count = observed_transfer_count
                .checked_add(1)
                .ok_or(DealerSbfError::Custody)?;
        }
    }
    if observed_transfer_count != prepared.resources.custody_transfer_count
        || prepared.resources.requires_custody() != (observed_transfer_count != 0)
        || (observed_transfer_count != 0 && prepared.resources.custody_role_count() < 2)
    {
        return Err(DealerSbfError::AccountFrame.into());
    }
    verify_final_custody_projection(accounts, prepared)
}

fn verify_final_custody_projection(
    accounts: &[AccountInfo<'_>],
    prepared: &Prepared,
) -> ProgramResult {
    if read_token(accounts, DEALER_QUOTE)?.amount != prepared.post_quote_custody
        || read_token(accounts, FEE_VAULT)?.amount != prepared.post_fee_custody
        || read_token(accounts, LIVENESS_VAULT)?.amount != prepared.post_liveness_custody
    {
        return Err(DealerSbfError::Custody.into());
    }
    Ok(())
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn execute_one_custody_child(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    prepared: &Prepared,
    transfer: dclutch_dealer_codec::CustodyTransfer,
    index: usize,
    expected_revision: u64,
    generation: u64,
) -> ProgramResult {
    let source_index = prepared.frame.token_account(transfer.source)?;
    let destination_index = prepared.frame.token_account(transfer.destination)?;
    let request = prepare_custody_request(
        program_id,
        accounts,
        prepared,
        transfer,
        index,
        expected_revision,
        generation,
    )?;
    let request_bytes = request.to_bytes().map_err(|_| DealerSbfError::Custody)?;
    let request_digest = hashv(&[&request_bytes]).to_bytes();
    authenticate_caller_authority(
        program_id,
        accounts,
        prepared.frame.custody_authority(index)?,
        prepared,
        accounts[STATE].key.to_bytes(),
        request_digest,
    )?;
    invoke_custody(
        program_id,
        accounts,
        prepared,
        request,
        &request_bytes,
        request_digest,
        index,
        source_index,
        destination_index,
    )
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn invoke_custody(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    prepared: &Prepared,
    request: CustodyRequestV1,
    request_bytes: &[u8],
    request_digest: [u8; 32],
    transfer_index: usize,
    source_index: usize,
    destination_index: usize,
) -> ProgramResult {
    let source_before = read_token(accounts, source_index)?.amount;
    let destination_before = read_token(accounts, destination_index)?.amount;
    let custody_authority_seeds = CustodyAuthoritySeedsV1::from_request(request);
    let expected_custody_authority = Pubkey::find_program_address(
        &custody_authority_seeds.as_slices(),
        accounts[CUSTODY_PROGRAM].key,
    )
    .0;
    if accounts[CUSTODY_TRANSFER_AUTHORITY].key != &expected_custody_authority {
        return Err(DealerSbfError::Custody.into());
    }
    let caller_authority = prepared.frame.custody_authority(transfer_index)?;
    let replay = prepared.frame.custody_replay()?;
    let instruction = Instruction {
        program_id: *accounts[CUSTODY_PROGRAM].key,
        accounts: Vec::from([
            AccountMeta::new_readonly(*accounts[caller_authority].key, true),
            AccountMeta::new_readonly(*accounts[ACTIVATION_CACHE].key, false),
            AccountMeta::new_readonly(*accounts[REGISTRY_PROGRAM].key, false),
            AccountMeta::new_readonly(*accounts[TRADING_PROGRAM].key, false),
            AccountMeta::new_readonly(*accounts[TRADING_PROGRAMDATA].key, false),
            AccountMeta::new_readonly(*accounts[REALM].key, false),
            AccountMeta::new(*accounts[replay].key, false),
            AccountMeta::new_readonly(*accounts[COLLATERAL_MINT].key, false),
            AccountMeta::new(*accounts[source_index].key, false),
            AccountMeta::new(*accounts[destination_index].key, false),
            AccountMeta::new_readonly(*accounts[CUSTODY_TRANSFER_AUTHORITY].key, false),
            AccountMeta::new_readonly(*accounts[TOKEN_PROGRAM].key, false),
        ]),
        data: request_bytes.to_vec(),
    };
    let child_accounts = [
        accounts[caller_authority].clone(),
        accounts[ACTIVATION_CACHE].clone(),
        accounts[REGISTRY_PROGRAM].clone(),
        accounts[TRADING_PROGRAM].clone(),
        accounts[TRADING_PROGRAMDATA].clone(),
        accounts[REALM].clone(),
        accounts[replay].clone(),
        accounts[COLLATERAL_MINT].clone(),
        accounts[source_index].clone(),
        accounts[destination_index].clone(),
        accounts[CUSTODY_TRANSFER_AUTHORITY].clone(),
        accounts[TOKEN_PROGRAM].clone(),
        accounts[CUSTODY_PROGRAM].clone(),
    ];
    invoke_as_trading(
        program_id,
        accounts,
        caller_authority,
        prepared,
        accounts[STATE].key.to_bytes(),
        request_digest,
        &instruction,
        &child_accounts,
        DealerSbfError::Custody,
    )?;
    verify_custody_receipt(
        accounts,
        request,
        request_digest,
        source_index,
        destination_index,
        source_before,
        destination_before,
        replay,
    )
}

#[allow(clippy::too_many_arguments)]
fn verify_custody_receipt(
    accounts: &[AccountInfo<'_>],
    request: CustodyRequestV1,
    request_digest: [u8; 32],
    source_index: usize,
    destination_index: usize,
    source_before: u64,
    destination_before: u64,
    replay: usize,
) -> ProgramResult {
    let (producer, bytes) = get_return_data().ok_or(DealerSbfError::Custody)?;
    if producer != *accounts[CUSTODY_PROGRAM].key {
        return Err(DealerSbfError::Custody.into());
    }
    let receipt = CustodyReceiptV1::decode(&bytes).map_err(|_| DealerSbfError::Custody)?;
    let replay_data = accounts[replay]
        .try_borrow_data()
        .map_err(|_| DealerSbfError::Custody)?;
    let replay_digest = hashv(&[&replay_data]).to_bytes();
    drop(replay_data);
    receipt
        .verify_for(request, request_digest, replay_digest)
        .map_err(|_| DealerSbfError::Custody)?;
    let source_after = read_token(accounts, source_index)?.amount;
    let destination_after = read_token(accounts, destination_index)?.amount;
    if receipt.evidence.source_before != source_before
        || receipt.evidence.destination_before != destination_before
        || receipt.evidence.source_after != source_after
        || receipt.evidence.destination_after != destination_after
    {
        return Err(DealerSbfError::Custody.into());
    }
    Ok(())
}

fn authenticate_caller_authority(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    authority_index: usize,
    prepared: &Prepared,
    context: [u8; 32],
    role_request_digest: [u8; 32],
) -> ProgramResult {
    let release_set =
        ContentId::new(prepared.policy.release_set_id).map_err(|_| DealerSbfError::Release)?;
    let seeds = CallerAuthoritySeedsV1::new(
        release_set,
        prepared.policy.market_id,
        ExecutionRoleV1::Trading,
        context,
        role_request_digest,
    )
    .map_err(|_| DealerSbfError::Release)?;
    let expected = Pubkey::find_program_address(&seeds.as_slices(), program_id).0;
    if accounts[authority_index].key != &expected {
        return Err(DealerSbfError::AccountIdentity.into());
    }
    Ok(())
}

#[inline(never)]
fn prepare_claims_packet(
    accounts: &[AccountInfo<'_>],
    prepared: &Prepared,
) -> Result<Option<ClaimsPacket>, ProgramError> {
    let (action, source_index, destination_index, source_owner, destination_owner) =
        match prepared.plan.claim {
            ClaimAction::None => return Ok(None),
            ClaimAction::Transfer {
                side: Side::TakerBuys,
                ..
            } => (
                ClaimsAction::TransferNative,
                prepared.frame.dealer_position()?,
                prepared.frame.actor_position()?,
                prepared.policy.dealer_id,
                accounts[ACTOR].key.to_bytes(),
            ),
            ClaimAction::Transfer {
                side: Side::TakerSells,
                ..
            } => (
                ClaimsAction::TransferNative,
                prepared.frame.actor_position()?,
                prepared.frame.dealer_position()?,
                accounts[ACTOR].key.to_bytes(),
                prepared.policy.dealer_id,
            ),
            ClaimAction::Redeem { .. } => (
                ClaimsAction::RedeemNativeTerminal,
                prepared.frame.dealer_position()?,
                prepared.frame.claims_program()?,
                prepared.policy.dealer_id,
                [0; 32],
            ),
            ClaimAction::AdjustLiquidity { add: true, .. } => (
                ClaimsAction::TransferNative,
                prepared.frame.actor_position()?,
                prepared.frame.dealer_position()?,
                accounts[ACTOR].key.to_bytes(),
                prepared.policy.dealer_id,
            ),
            ClaimAction::AdjustLiquidity { add: false, .. } => (
                ClaimsAction::TransferNative,
                prepared.frame.dealer_position()?,
                prepared.frame.actor_position()?,
                prepared.policy.dealer_id,
                accounts[ACTOR].key.to_bytes(),
            ),
        };
    let outcomes = u32::from(prepared.policy.outcome_count);
    let market_data = accounts[prepared.frame.claims_market()?]
        .try_borrow_data()
        .map_err(|_| DealerSbfError::Claims)?;
    let expected_market_revision =
        market_revision(&market_data).map_err(|_| DealerSbfError::Claims)?;
    drop(market_data);
    let source_data = accounts[source_index]
        .try_borrow_data()
        .map_err(|_| DealerSbfError::Claims)?;
    let expected_source_revision =
        position_revision(&source_data, outcomes).map_err(|_| DealerSbfError::Claims)?;
    drop(source_data);
    let expected_destination_revision = if destination_index == prepared.frame.claims_program()? {
        NO_POSITION_REVISION
    } else {
        let destination_data = accounts[destination_index]
            .try_borrow_data()
            .map_err(|_| DealerSbfError::Claims)?;
        let revision =
            position_revision(&destination_data, outcomes).map_err(|_| DealerSbfError::Claims)?;
        drop(destination_data);
        revision
    };
    let (outcome, quantity) = match prepared.plan.claim {
        ClaimAction::None => return Err(DealerSbfError::Claims.into()),
        ClaimAction::Transfer {
            outcome, quantity, ..
        }
        | ClaimAction::Redeem {
            outcome, quantity, ..
        }
        | ClaimAction::AdjustLiquidity {
            outcome, quantity, ..
        } => (usize::from(outcome), quantity),
    };
    let mut quantities = [0_u8; MAX_OUTCOMES * 8];
    let start = outcome.checked_mul(8).ok_or(DealerSbfError::Claims)?;
    quantities
        .get_mut(start..start + 8)
        .ok_or(DealerSbfError::Claims)?
        .copy_from_slice(&quantity.to_le_bytes());
    let tail_len = usize::try_from(outcomes)
        .ok()
        .and_then(|count| count.checked_mul(8))
        .ok_or(DealerSbfError::Claims)?;
    let plan = ClaimsPlanV1::new(
        action,
        CallerRole::Trading,
        prepared.policy.release_set_id,
        prepared.policy.market_id,
        prepared.parent_request_digest,
        source_owner,
        destination_owner,
        expected_market_revision,
        expected_source_revision,
        expected_destination_revision,
        outcomes,
        quantities.get(..tail_len).ok_or(DealerSbfError::Claims)?,
    )
    .map_err(|_| DealerSbfError::Claims)?;
    let len = CLAIMS_PLAN_HEADER_BYTES_V1
        .checked_add(tail_len)
        .ok_or(DealerSbfError::Claims)?;
    let mut packet = ClaimsPacket {
        bytes: [0; CLAIMS_PACKET_MAX_BYTES],
        len,
        digest: [0; 32],
    };
    plan.encode_into(packet.bytes.get_mut(..len).ok_or(DealerSbfError::Claims)?)
        .map_err(|_| DealerSbfError::Claims)?;
    packet.digest = hashv(&[packet.bytes.get(..len).ok_or(DealerSbfError::Claims)?]).to_bytes();
    Ok(Some(packet))
}

#[allow(clippy::too_many_arguments)]
fn prepare_custody_request(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    prepared: &Prepared,
    transfer: dclutch_dealer_codec::CustodyTransfer,
    transfer_index: usize,
    expected_revision: u64,
    generation: u64,
) -> Result<CustodyRequestV1, ProgramError> {
    let source_index = prepared.frame.token_account(transfer.source)?;
    let destination_index = prepared.frame.token_account(transfer.destination)?;
    let source_compartment = custody_compartment(transfer.source);
    let destination_compartment = custody_compartment(transfer.destination);
    let source_owner =
        external_owner(accounts, prepared.policy, transfer.source).unwrap_or([0; 32]);
    let destination_owner =
        external_owner(accounts, prepared.policy, transfer.destination).unwrap_or([0; 32]);
    let resulting_revision = expected_revision
        .checked_add(1)
        .ok_or(DealerSbfError::Custody)?;
    CustodyRequestV1 {
        operation: OperationV1::Transfer,
        caller_role: CallerRoleV1::Trading,
        source_compartment,
        destination_compartment,
        release_set: prepared.policy.release_set_id,
        market: prepared.policy.market_id,
        realm: prepared.realm_id,
        context: accounts[STATE].key.to_bytes(),
        caller_program: program_id.to_bytes(),
        semantic: ContextV1 {
            candidate: prepared.pre_active_candidate_id,
            source_owner,
            destination_owner,
            order: accounts[ACTOR].key.to_bytes(),
            parent_request_digest: prepared.parent_request_digest,
            order_nonce: prepared.pre_state_revision,
            generation,
            page_index: 0,
            execution_index: 0,
            transfer_index: u16::try_from(transfer_index).map_err(|_| DealerSbfError::Custody)?,
        },
        source: accounts[source_index].key.to_bytes(),
        destination: accounts[destination_index].key.to_bytes(),
        source_vault_context: custody_vault_context(
            source_compartment,
            accounts[STATE].key.to_bytes(),
            prepared.policy.market_id,
        ),
        destination_vault_context: custody_vault_context(
            destination_compartment,
            accounts[STATE].key.to_bytes(),
            prepared.policy.market_id,
        ),
        mint: accounts[COLLATERAL_MINT].key.to_bytes(),
        token_program: accounts[TOKEN_PROGRAM].key.to_bytes(),
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision,
        resulting_revision,
        amount: transfer.amount,
        rent_lamports: 0,
    }
    .to_bytes()
    .and_then(|bytes| CustodyRequestV1::decode(&bytes))
    .map_err(|_| DealerSbfError::Custody.into())
}

fn external_owner(
    accounts: &[AccountInfo<'_>],
    policy: Policy,
    role: CustodyRole,
) -> Option<[u8; 32]> {
    match role {
        CustodyRole::TakerQuote | CustodyRole::Executor => Some(accounts[ACTOR].key.to_bytes()),
        CustodyRole::DealerOwner => Some(policy.dealer_id),
        CustodyRole::UnwindRecipient => Some(policy.unwind_recipient_id),
        CustodyRole::FeeRecipient => Some(policy.fee_recipient_id),
        CustodyRole::DealerQuote
        | CustodyRole::FeeVault
        | CustodyRole::LivenessVault
        | CustodyRole::MarketHoard => None,
    }
}

const fn custody_vault_context(
    compartment: CompartmentV1,
    dealer_state: [u8; 32],
    market: [u8; 32],
) -> [u8; 32] {
    match compartment {
        CompartmentV1::External | CompartmentV1::None => [0; 32],
        CompartmentV1::HoardPrincipal => market,
        CompartmentV1::TradingPrincipal
        | CompartmentV1::FeeVault
        | CompartmentV1::LivenessVault
        | CompartmentV1::Settlement
        | CompartmentV1::SeriesEscrow
        | CompartmentV1::RecoveryReserve => dealer_state,
    }
}

const fn custody_compartment(role: CustodyRole) -> CompartmentV1 {
    match role {
        CustodyRole::DealerQuote => CompartmentV1::TradingPrincipal,
        CustodyRole::TakerQuote
        | CustodyRole::Executor
        | CustodyRole::DealerOwner
        | CustodyRole::UnwindRecipient
        | CustodyRole::FeeRecipient => CompartmentV1::External,
        CustodyRole::FeeVault => CompartmentV1::FeeVault,
        CustodyRole::LivenessVault => CompartmentV1::LivenessVault,
        CustodyRole::MarketHoard => CompartmentV1::HoardPrincipal,
    }
}

#[inline(never)]
fn commit_reinterpreted_state(
    accounts: &[AccountInfo<'_>],
    prepared: &Prepared,
    instruction_data: &[u8],
) -> ProgramResult {
    let policy_data = accounts[POLICY]
        .try_borrow_data()
        .map_err(|_| DealerSbfError::Commit)?;
    let active_data = accounts[ACTIVE_CANDIDATE]
        .try_borrow_data()
        .map_err(|_| DealerSbfError::Commit)?;
    let pending_data = if accounts[PENDING_CANDIDATE].key == accounts[TRADING_PROGRAM].key {
        None
    } else {
        Some(
            accounts[PENDING_CANDIDATE]
                .try_borrow_data()
                .map_err(|_| DealerSbfError::Commit)?,
        )
    };
    let proposed_data = if accounts[PROPOSED_CANDIDATE].key == accounts[TRADING_PROGRAM].key {
        None
    } else {
        Some(
            accounts[PROPOSED_CANDIDATE]
                .try_borrow_data()
                .map_err(|_| DealerSbfError::Commit)?,
        )
    };
    let state_data = accounts[STATE]
        .try_borrow_data()
        .map_err(|_| DealerSbfError::Commit)?;
    let transition = interpret(Inputs {
        policy: &policy_data,
        active_candidate: &active_data,
        pending_candidate: pending_data.as_ref().map(|data| &data[..]),
        proposed_candidate: proposed_data.as_ref().map(|data| &data[..]),
        release_receipt: &prepared.normalized_receipt,
        state: &state_data,
        request: instruction_data,
    })
    .map_err(|_| DealerSbfError::Commit)?;
    if transition.plan != prepared.plan
        || transition.post.quote_custody != prepared.post_quote_custody
        || transition.post.fee_custody != prepared.post_fee_custody
        || transition.post.liveness_custody != prepared.post_liveness_custody
        || ChildResources::derive(transition.plan).map_err(|_| DealerSbfError::Commit)?
            != prepared.resources
    {
        return Err(DealerSbfError::Commit.into());
    }
    drop(policy_data);
    drop(active_data);
    drop(pending_data);
    drop(proposed_data);
    drop(state_data);
    write_state(accounts, transition.post)
}

#[inline(never)]
fn write_state(accounts: &[AccountInfo<'_>], post: State) -> ProgramResult {
    let bytes = post.to_bytes().map_err(|_| DealerSbfError::Commit)?;
    let mut state = accounts[STATE]
        .try_borrow_mut_data()
        .map_err(|_| DealerSbfError::Commit)?;
    if state.len() != bytes.len() {
        return Err(DealerSbfError::Commit.into());
    }
    state.copy_from_slice(&bytes);
    Ok(())
}

#[cfg(test)]
mod sparse_frame_tests {
    use super::*;
    use dclutch_dealer_codec::CustodyTransfer;

    const fn transfer(
        source: CustodyRole,
        destination: CustodyRole,
        amount: u64,
    ) -> Option<CustodyTransfer> {
        Some(CustodyTransfer {
            source,
            destination,
            amount,
        })
    }

    fn assert_exact_count(plan: Plan, expected: usize) {
        let resources = ChildResources::derive(plan).expect("canonical child plan");
        assert!(SparseFrame::parse(expected, resources).is_ok());
        assert!(SparseFrame::parse(expected - 1, resources).is_err());
        assert!(SparseFrame::parse(expected + 1, resources).is_err());
    }

    #[test]
    fn action_shaped_frames_refuse_missing_and_surplus_accounts() {
        let schedule = Plan {
            claim: ClaimAction::None,
            custody: [
                transfer(CustodyRole::LivenessVault, CustodyRole::DealerOwner, 3),
                transfer(CustodyRole::DealerOwner, CustodyRole::LivenessVault, 7),
                None,
            ],
        };
        assert_exact_count(schedule, 27);

        let activate = Plan {
            claim: ClaimAction::None,
            custody: [
                transfer(CustodyRole::LivenessVault, CustodyRole::DealerOwner, 3),
                None,
                None,
            ],
        };
        assert_exact_count(activate, 26);

        let fill = Plan {
            claim: ClaimAction::Transfer {
                side: Side::TakerBuys,
                outcome: 1,
                quantity: 5,
            },
            custody: [
                transfer(CustodyRole::TakerQuote, CustodyRole::DealerQuote, 91),
                transfer(CustodyRole::TakerQuote, CustodyRole::FeeVault, 3),
                transfer(CustodyRole::LivenessVault, CustodyRole::Executor, 2),
            ],
        };
        assert_exact_count(fill, 35);

        let terminal_without_pending_refund = Plan {
            claim: ClaimAction::None,
            custody: [None; 3],
        };
        assert_exact_count(terminal_without_pending_refund, 23);

        let terminal_with_pending_refund = Plan {
            claim: ClaimAction::None,
            custody: [
                transfer(CustodyRole::LivenessVault, CustodyRole::DealerOwner, 3),
                None,
                None,
            ],
        };
        assert_exact_count(terminal_with_pending_refund, 26);

        let unwind = Plan {
            claim: ClaimAction::Redeem {
                outcome: 1,
                quantity: 5,
                payout: 5,
            },
            custody: [
                transfer(CustodyRole::MarketHoard, CustodyRole::DealerQuote, 5),
                transfer(CustodyRole::LivenessVault, CustodyRole::Executor, 2),
                None,
            ],
        };
        assert_exact_count(unwind, 33);

        let retire = Plan {
            claim: ClaimAction::None,
            custody: [
                transfer(CustodyRole::DealerQuote, CustodyRole::UnwindRecipient, 11),
                transfer(CustodyRole::FeeVault, CustodyRole::FeeRecipient, 7),
                transfer(CustodyRole::LivenessVault, CustodyRole::DealerOwner, 3),
            ],
        };
        assert_exact_count(retire, 30);

        for add in [true, false] {
            let native_claim_liquidity = Plan {
                claim: ClaimAction::AdjustLiquidity {
                    add,
                    outcome: 1,
                    quantity: 5,
                },
                custody: [None; 3],
            };
            assert_exact_count(native_claim_liquidity, 29);
        }

        let quote_liquidity = Plan {
            claim: ClaimAction::None,
            custody: [
                transfer(CustodyRole::DealerOwner, CustodyRole::DealerQuote, 5),
                None,
                None,
            ],
        };
        assert_exact_count(quote_liquidity, 26);
    }
}
