#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Thin SVM refinement of the canonical multiprogram Custody contract.

extern crate alloc;

use alloc::vec::Vec;
use core::convert::TryFrom;

use dclutch_market::capability_manifest::funding::funded_rent_persists_v1;
use dclutch_core_contract::ContentId;
use dclutch_custody::{
    CUSTODY_BUMP_RELAY_BYTES_V1, CUSTODY_RECEIPT_BYTES_V1, CUSTODY_REPLAY_BYTES_V1,
    CUSTODY_REQUEST_BYTES_V1, CallerRoleV1, CompartmentV1, CustodyAuthoritySeedsV1,
    CustodyFrameSpecV1, CustodyReceiptV1, CustodyReplaySeedsV1, CustodyReplayV1, CustodyRequestV1,
    CustodyVaultSeedsV1, DELEGATED_CUSTODY_REQUEST_BYTES_V2, DELEGATED_CUSTODY_REQUEST_MAGIC_V2,
    OperationV1, PROJECTED_CUSTODY_REQUEST_BYTES_V1, PROJECTED_CUSTODY_REQUEST_MAGIC_V1,
    ProjectedCustodyRequestV1, ReceiptEvidenceV1, classify_premarket_series_escrow_v1,
};

mod delegated;
mod projected;
mod retirement_replay_handoff_v1;

/// One diagnostic phase mark: a label and the transaction meter's remaining CU.
///
/// A child program reached by CPI is ONE number in its caller's log, and one
/// number cannot say whether it was spent on work only this program can do or
/// on re-deriving what its caller already authenticated. This program is that
/// number twice in the Dealer partial equity Remove -- 116,203 CU for a cash
/// leg whose Token-2022 CPI inside it costs 105, and the same again for a merge
/// the transaction never reaches. See
/// `docs/design/DEALER_PARTIAL_REMOVE_COMPUTE_2026_09_02.md`.
///
/// The syscalls and the `#[inline(never)]` that guards them live in
/// `dclutch-sbf-runtime::cu_checkpoint`, which says why. What stays here is the feature name
/// a build line names and the domain prefix a log reader greps.
#[cfg(feature = "custody-cu-profile")]
macro_rules! custody_cu_checkpoint {
    ($phase:literal) => {
        dclutch_sbf_runtime::cu_checkpoint::cu_checkpoint(concat!("dclutch-custody-cu:", $phase))
    };
}

#[cfg(not(feature = "custody-cu-profile"))]
macro_rules! custody_cu_checkpoint {
    ($phase:literal) => {};
}

use dclutch_market::{CoreState, MarketCoreStateSeedsV2, STATE_BYTES};
use dclutch_market::realm::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_BYTES, REALM_SCHEMA_RELEASE_ID_V1, RealmV1,
};
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry::activation_auth_v1::{
    authenticate_activation_cache_identity_v1, require_cache_account, require_readonly_frame,
};
use dclutch_registry::{ACTIVATION_PDA_DOMAIN_V1, ActivatedExecutionReleaseSetViewV1};
use dclutch_registry::svm::continuation_v1::{
    REGISTRY_CONTINUATION_REQUEST_BYTES_V1, RegistryContinuationAdmissionSeedsV1,
    RegistryContinuationRequestV1,
};
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_custody::token_svm::{
    AuthorityRole, COption, ExactTransferInput, ExactTransferProfileV1,
    PRODUCTION_ADAPTER_RELEASES, close_account, initialize_account3, transfer_checked,
};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    program::{invoke, invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::{allocate, assign, create_account, transfer};

/// Exact common prefix length.
pub const COMMON_ACCOUNT_COUNT_V1: usize =
    dclutch_custody::CUSTODY_COMMON_ACCOUNT_COUNT_V1 as usize;
/// Exact `InitializeReplay` account count.
pub const INITIALIZE_REPLAY_ACCOUNT_COUNT_V1: usize =
    dclutch_custody::INITIALIZE_REPLAY_ACCOUNT_COUNT_V1 as usize;
/// Exact `OpenVault` account count.
pub const OPEN_VAULT_ACCOUNT_COUNT_V1: usize =
    dclutch_custody::OPEN_VAULT_ACCOUNT_COUNT_V1 as usize;
/// Exact `Transfer` account count.
pub const TRANSFER_ACCOUNT_COUNT_V1: usize =
    dclutch_custody::TRANSFER_ACCOUNT_COUNT_V1 as usize;
/// Exact `CloseVault` account count.
pub const CLOSE_VAULT_ACCOUNT_COUNT_V1: usize =
    dclutch_custody::CLOSE_VAULT_ACCOUNT_COUNT_V1 as usize;
/// Exact `CloseReplay` account count.
pub const CLOSE_REPLAY_ACCOUNT_COUNT_V1: usize =
    dclutch_custody::CLOSE_REPLAY_ACCOUNT_COUNT_V1 as usize;

const CALLER_AUTHORITY: usize = 0;
const CORE_MARKET: usize = 1;
const ACTIVATION_CACHE: usize = 2;
const REGISTRY_PROGRAM: usize = 3;
const CALLER_PROGRAM: usize = 4;
const CALLER_PROGRAMDATA: usize = 5;
const REALM: usize = 6;
const REALM_STAGING: usize = 7;
const REPLAY: usize = 8;

/// Stable refusal from the thin Custody SBF adapter.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CustodySbfError {
    /// Instruction bytes did not decode as the one generated request.
    Instruction = 0x6000,
    /// Account count, order, privileges, or aliases were not exact.
    AccountFrame = 0x6001,
    /// Registry CPI, producer, receipt, release, role, or caller refused.
    Release = 0x6002,
    /// Caller authority was not the release-pinned role PDA signer.
    CallerAuthority = 0x6003,
    /// Realm content, PDA, owner, Mint, token program, or adapter release refused.
    Realm = 0x6004,
    /// Replay PDA, owner, bytes, or revision refused.
    Replay = 0x6005,
    /// Vault PDA, token state, or authority policy refused.
    TokenState = 0x6006,
    /// Rent, payer, System program, or account creation refused.
    Create = 0x6007,
    /// Exact token or close-account CPI refused.
    TokenCpi = 0x6008,
    /// Exact CPI postcondition or checked balance arithmetic refused.
    Postcondition = 0x6009,
    /// Replay state could not be committed after all effects succeeded.
    Commit = 0x600A,
    /// An expiry-gated terminal was attempted at the wrong time.
    ///
    /// The kernel has always distinguished this from [`Self::Replay`]; this
    /// program used to flatten both into it, so a projection refusing an early
    /// unwind reported "replay PDA, owner, bytes, or revision refused" - which
    /// is not what happened and is not what a reader needs to know. For a
    /// terminal whose entire safety property is that it refuses while the
    /// founding is still satisfiable, the refusal has to be able to say so.
    Expiry = 0x600B,
    /// The release's pinned deployment slot moved: the substrate was upgraded.
    /// Every open market on the superseded release generation refuses until a
    /// re-release re-authenticates the new deployment and re-pins its slot.
    ///
    /// Decision 0012. Not a corrupted account and not an attack: the exact
    /// upgrade authority the release names shipped new bytes, so the cached
    /// authentication no longer describes what is deployed.
    ReleaseSuperseded = 0x600C,
    /// Withdrawn with the Dealer scenario reservation route. Never raised;
    /// the discriminant is not reused.
    ReservationRecord = 0x600D,
    /// Withdrawn with the Dealer scenario reservation route. Never raised;
    /// the discriminant is not reused.
    ReservationIdentity = 0x600E,
    /// Withdrawn with the Dealer scenario reservation route. Never raised;
    /// the discriminant is not reused.
    ReservationFrame = 0x600F,
    /// Withdrawn with the Dealer scenario reservation route. Never raised;
    /// the discriminant is not reused.
    ReservationEscrowPrestate = 0x6010,
    /// A Transfer named `HoardPrincipal -> FeeVault`.
    ///
    /// Split out of [`Self::Instruction`], which means "these bytes did not
    /// decode" -- and this wire decodes perfectly. It is refused because the
    /// Hoard is the collateral every outstanding claim is redeemed against and
    /// a fee is revenue, which is the cross-subsidy C-10 forbids and
    /// `AGENTS.md` states as an invariant. A reader who sees `Instruction` goes
    /// looking for a malformed request and finds a well-formed one; a reader
    /// who sees this code is told which invariant stopped them.
    ///
    /// Ruled 2026-09-04 (C-11 D1 item 5). Contract twin:
    /// `dclutch_custody::Error::ForbiddenCompartmentPair`; Lean twin:
    /// `hoard_principal_never_funds_the_fee_vault` in `CustodyAbi.lean`.
    ForbiddenCompartmentPair = 0x6011,
}

dclutch_refusal_registry::pin_refusal_band!(
    CustodySbfError,
    dclutch_refusal_registry::CUSTODY_REFUSAL_BASE,
    [
        Instruction,
        AccountFrame,
        Release,
        CallerAuthority,
        Realm,
        Replay,
        TokenState,
        Create,
        TokenCpi,
        Postcondition,
        Commit,
        Expiry,
        ReleaseSuperseded,
        ReservationRecord,
        ReservationIdentity,
        ReservationFrame,
        ReservationEscrowPrestate,
        ForbiddenCompartmentPair
    ]
);

/// Name an activation-cache refusal, keeping the superseded case actionable.
///
/// Decision 0012: a moved deployment slot means the substrate was upgraded and
/// this release generation is finished. The remedy is a re-release, not an
/// investigation, so it does not fold into the generic Release refusal.
impl From<dclutch_registry::activation_auth_v1::ActivationAuthErrorV1> for CustodySbfError {
    fn from(value: dclutch_registry::activation_auth_v1::ActivationAuthErrorV1) -> Self {
        match value {
            dclutch_registry::activation_auth_v1::ActivationAuthErrorV1::ReleaseSuperseded => {
                Self::ReleaseSuperseded
            }
            _ => Self::Release,
        }
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// Execute one exact replay, vault, transfer, or close effect.
#[inline(never)]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    custody_cu_checkpoint!("cu-enter");
    let (instruction_data, relay) = split_caller_authority_bump_v1(instruction_data);
    if instruction_data.len()
        == dclutch_custody::RETIREMENT_REPLAY_HANDOFF_REQUEST_BYTES_V1
        && instruction_data
            .get(..dclutch_custody::RETIREMENT_REPLAY_HANDOFF_REQUEST_MAGIC_V1.len())
            == Some(dclutch_custody::RETIREMENT_REPLAY_HANDOFF_REQUEST_MAGIC_V1.as_slice())
    {
        let request =
            dclutch_custody::RetirementReplayHandoffRequestV1::decode(instruction_data)
                .map_err(|_| CustodySbfError::Instruction)?;
        return retirement_replay_handoff_v1::process(
            program_id,
            accounts,
            request,
            instruction_data,
        );
    }
    if instruction_data.len() == DELEGATED_CUSTODY_REQUEST_BYTES_V2
        && instruction_data.get(..DELEGATED_CUSTODY_REQUEST_MAGIC_V2.len())
            == Some(DELEGATED_CUSTODY_REQUEST_MAGIC_V2.as_slice())
    {
        return delegated::process(program_id, accounts, instruction_data, relay);
    }
    if instruction_data.len() == PROJECTED_CUSTODY_REQUEST_BYTES_V1
        && instruction_data.get(..PROJECTED_CUSTODY_REQUEST_MAGIC_V1.len())
            == Some(PROJECTED_CUSTODY_REQUEST_MAGIC_V1.as_slice())
    {
        let request = ProjectedCustodyRequestV1::decode(instruction_data)
            .map_err(|_| CustodySbfError::Instruction)?;
        return projected::process(program_id, accounts, request, instruction_data);
    }
    let (request_bytes, continuation) = split_registry_continuation(instruction_data)?;
    let request = CustodyRequestV1::decode(request_bytes).map_err(|error| match error {
        // The one refusal in `validate` that is an economic verdict rather
        // than a decode failure keeps its own word on the wire. Every other
        // cause is genuinely "these bytes are not the request", which is
        // what `Instruction` says.
        dclutch_custody::Error::ForbiddenCompartmentPair => {
            CustodySbfError::ForbiddenCompartmentPair
        }
        _ => CustodySbfError::Instruction,
    })?;
    require_account_count(accounts, request.operation, continuation.is_some())?;
    let request_digest = hash(request_bytes).to_bytes();
    custody_cu_checkpoint!("cu-decoded");
    // ONE borrow and ONE decode of the activation cache for the whole
    // invocation: a decode with its identity conjuncts costs 25,000 to 31,000
    // CU, and `authenticate_activation_cache_identity_v1` once, then every
    // role read out of the SAME view, is the pair
    // `dclutch-registry::activation_auth_v1` names. The guard is held across
    // the frame and the realm and dropped before dispatch, so nothing below
    // reads a view whose bytes could have moved underneath it.
    let registry = account(accounts, REGISTRY_PROGRAM)?;
    let cache_account = account(accounts, ACTIVATION_CACHE)?;
    require_cache_account(registry.key, cache_account).map_err(CustodySbfError::from)?;
    let cache_data = cache_account
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Release)?;
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&cache_data)
        .map_err(|_| CustodySbfError::Release)?;
    custody_cu_checkpoint!("cf-cache-decode");
    authenticate_activation_cache_identity_v1(
        registry,
        cache_account,
        &request.release_set,
        activated,
    )
    .map_err(CustodySbfError::from)?;
    custody_cu_checkpoint!("cf-cache-identity");
    let market = authenticate_series_aware_common_frame(
        program_id,
        accounts,
        request,
        request_digest,
        continuation,
        relay,
        activated,
    )?;
    custody_cu_checkpoint!("cu-common-frame");
    let realm = match market {
        AuthenticatedMarketAdmissionV1::Live(market) => {
            authenticate_realm(program_id, accounts, request, market.state, activated)?
        }
        AuthenticatedMarketAdmissionV1::Premarket { .. } => {
            authenticate_premarket_realm(program_id, accounts, request, activated)?
        }
    };
    custody_cu_checkpoint!("cu-realm");
    drop(cache_data);
    let outcome = match request.operation {
        OperationV1::InitializeReplay => {
            initialize_replay(program_id, accounts, request, request_digest)
        }
        OperationV1::OpenVault => open_vault(program_id, accounts, request, request_digest, realm),
        OperationV1::Transfer => {
            execute_transfer(program_id, accounts, request, request_digest, realm, relay)
        }
        OperationV1::CloseVault => {
            close_vault(program_id, accounts, request, request_digest, realm)
        }
        OperationV1::CloseReplay => close_replay(program_id, accounts, request, request_digest),
    };
    custody_cu_checkpoint!("cu-return");
    outcome
}

/// Strip the optional caller-authority bump the parent appended after its
/// request, and return the instruction this program's own dispatch sees.
///
/// # Why a byte AFTER the request, and not a reserved byte inside it
///
/// `CallerAuthoritySeedsV1`'s last seed is `hash(request_bytes)`. A bump written
/// into the request would change that digest, which changes the address, which
/// changes the bump: the obvious carrier has no fixed point. A byte after the
/// request is outside the loop, because the digest is taken over the request
/// only -- pinned by
/// `the_caller_authority_digest_covers_the_request_prefix_only` below, so that
/// widening the digest to cover the whole instruction data meets a red row that
/// explains what it breaks.
///
/// # Why this is length-driven, and why that is safe
///
/// This program dispatches on EXACT LENGTH, so the carry has to be legible to
/// the dispatch before the dispatch runs. Each accepted length plus one is a
/// length no route of this program otherwise accepts, which the compile-time
/// assertions below make a checked fact rather than a reading of the file. A
/// wire that is not `known + 1` is handed on untouched and refuses exactly
/// where it used to.
///
/// The byte is not an authority: `authenticate_common_frame` reproduces the
/// address from it and compares against the account at coordinate 0, so a wrong
/// byte reproduces a different address and refuses. A caller that sends no
/// suffix gets the search this always used to do.
fn split_caller_authority_bump_v1(instruction_data: &[u8]) -> (&[u8], CustodyBumpRelayV1) {
    const V1: usize = dclutch_custody::CUSTODY_REQUEST_BYTES_V1;
    const CONTINUED: usize = V1 + REGISTRY_CONTINUATION_REQUEST_BYTES_V1;
    const DELEGATED: usize = DELEGATED_CUSTODY_REQUEST_BYTES_V2;
    const PROJECTED: usize = PROJECTED_CUSTODY_REQUEST_BYTES_V1;
    const HANDOFF: usize = dclutch_custody::RETIREMENT_REPLAY_HANDOFF_REQUEST_BYTES_V1;

    /// The routes whose handler READS a carried bump. The projected and
    /// retirement-handoff routes derive their caller authority elsewhere and
    /// are not on the hot path, so a byte after them would be stripped and then
    /// dropped -- worse than not stripping it.
    const CARRIED: [usize; 3] = [V1, CONTINUED, DELEGATED];
    /// Every exact length this program dispatches on, carried or not.
    const KNOWN: [usize; 5] = [V1, CONTINUED, DELEGATED, PROJECTED, HANDOFF];
    /// The suffix widths a carried wire may have, narrowest first.
    ///
    /// One byte is the original carrier and still means exactly what it meant:
    /// the caller authority only. Three is that byte plus the two addresses
    /// THIS program derives for itself and can carry from nowhere else -- the
    /// replay and the transfer authority. Both are searched once per
    /// transaction on a depth drawn from the participant keys, and neither can
    /// be stored: `CUSTODY_REPLAY_BYTES_V1` is exactly packed and Lean-emitted,
    /// so growing it by a byte would orphan every replay on chain. A relayed
    /// hint needs no migration at all.
    const WIDTHS: [usize; 2] = [1, CUSTODY_BUMP_RELAY_BYTES_V1];

    // No carried length at any accepted width may BE another route's exact
    // length -- a wire would dispatch as a different route with its tail eaten
    // -- and no two (route, width) pairs may land on the same total, or one
    // route's carried wire would be read at the other's width.
    const _: () = {
        let mut outer = 0;
        while outer < CARRIED.len() {
            let mut width = 0;
            while width < WIDTHS.len() {
                let total = CARRIED[outer] + WIDTHS[width];
                let mut inner = 0;
                while inner < KNOWN.len() {
                    assert!(
                        total != KNOWN[inner],
                        "a carried Custody wire would collide with another route's exact length"
                    );
                    inner += 1;
                }
                let mut other = 0;
                while other < CARRIED.len() {
                    let mut other_width = 0;
                    while other_width < WIDTHS.len() {
                        assert!(
                            (other == outer && other_width == width)
                                || total != CARRIED[other] + WIDTHS[other_width],
                            "two carried Custody wires would share one total length"
                        );
                        other_width += 1;
                    }
                    other += 1;
                }
                width += 1;
            }
            outer += 1;
        }
    };

    let mut width = 0;
    while width < WIDTHS.len() {
        let Some(carried) = instruction_data.len().checked_sub(WIDTHS[width]) else {
            width += 1;
            continue;
        };
        let mut index = 0;
        while index < CARRIED.len() {
            if CARRIED[index] == carried {
                let (request, suffix) = instruction_data.split_at(carried);
                return (request, CustodyBumpRelayV1::read(suffix));
            }
            index += 1;
        }
        width += 1;
    }
    (instruction_data, CustodyBumpRelayV1::ABSENT)
}

/// The bumps the parent mined so this program reproduces instead of searching.
///
/// Every field is a HINT and none is an authority. Each one is fed to a
/// `create_program_address` whose result is compared against the account this
/// frame was handed, so a wrong byte reproduces a different address and
/// refuses at the comparison that was always there. Zero is absent and the
/// reader searches exactly as it used to, which is what an unrelayed wire --
/// every wire emitted before this widened -- decodes to.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct CustodyBumpRelayV1 {
    /// Trading's caller-authority PDA. The original one-byte carrier.
    caller_authority: Option<u8>,
    /// This program's own replay PDA for the calling role and context.
    replay: Option<u8>,
    /// This program's own transfer authority for the Market and release set.
    transfer_authority: Option<u8>,
}

impl CustodyBumpRelayV1 {
    /// Nothing relayed: every reader searches, exactly as it used to.
    const ABSENT: Self = Self {
        caller_authority: None,
        replay: None,
        transfer_authority: None,
    };

    /// Read a one- or three-byte suffix in canonical order.
    ///
    /// Zero is not a bump any derivation produces, so it reads as absent rather
    /// than as a value that is certain to fail to reproduce.
    fn read(suffix: &[u8]) -> Self {
        let at = |index: usize| suffix.get(index).copied().filter(|bump| *bump != 0);
        Self {
            caller_authority: at(0),
            replay: at(1),
            transfer_authority: at(2),
        }
    }
}

fn split_registry_continuation(
    instruction_data: &[u8],
) -> Result<(&[u8], Option<RegistryContinuationRequestV1>), ProgramError> {
    if instruction_data.len() == dclutch_custody::CUSTODY_REQUEST_BYTES_V1 {
        return Ok((instruction_data, None));
    }
    let expected = dclutch_custody::CUSTODY_REQUEST_BYTES_V1
        .checked_add(REGISTRY_CONTINUATION_REQUEST_BYTES_V1)
        .ok_or(CustodySbfError::Instruction)?;
    if instruction_data.len() != expected {
        return Err(CustodySbfError::Instruction.into());
    }
    let (request, continuation) = instruction_data.split_at(CUSTODY_REQUEST_BYTES_V1);
    let continuation = RegistryContinuationRequestV1::decode(continuation)
        .map_err(|_| CustodySbfError::Release)?;
    Ok((request, Some(continuation)))
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn authenticate_common_frame(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CustodyRequestV1,
    request_digest: [u8; 32],
    continuation: Option<RegistryContinuationRequestV1>,
    relay: CustodyBumpRelayV1,
    activated: ActivatedExecutionReleaseSetViewV1<'_>,
) -> Result<CoreState, ProgramError> {
    let caller_authority = account(accounts, CALLER_AUTHORITY)?;
    let caller_program = account(accounts, CALLER_PROGRAM)?;
    let replay = account(accounts, REPLAY)?;

    if caller_program.key.to_bytes() != request.caller_program {
        return Err(CustodySbfError::Release.into());
    }
    let market = authenticate_market(accounts, request, activated)?;
    authenticate_common_frame_tail(
        program_id,
        accounts,
        request,
        request_digest,
        continuation,
        relay,
        activated,
        caller_authority,
        caller_program,
        replay,
    )?;
    Ok(market.state)
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn authenticate_series_aware_common_frame(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CustodyRequestV1,
    request_digest: [u8; 32],
    continuation: Option<RegistryContinuationRequestV1>,
    relay: CustodyBumpRelayV1,
    activated: ActivatedExecutionReleaseSetViewV1<'_>,
) -> Result<AuthenticatedMarketAdmissionV1, ProgramError> {
    let caller_authority = account(accounts, CALLER_AUTHORITY)?;
    let caller_program = account(accounts, CALLER_PROGRAM)?;
    let replay = account(accounts, REPLAY)?;

    if caller_program.key.to_bytes() != request.caller_program {
        return Err(CustodySbfError::Release.into());
    }
    custody_cu_checkpoint!("cf-accounts");
    let market = if try_authenticate_premarket_market(accounts, request)? {
        AuthenticatedMarketAdmissionV1::Premarket
    } else {
        AuthenticatedMarketAdmissionV1::Live(authenticate_market(accounts, request, activated)?)
    };
    custody_cu_checkpoint!("cf-market");
    authenticate_common_frame_tail(
        program_id,
        accounts,
        request,
        request_digest,
        continuation,
        relay,
        activated,
        caller_authority,
        caller_program,
        replay,
    )?;
    Ok(market)
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn authenticate_common_frame_tail(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CustodyRequestV1,
    request_digest: [u8; 32],
    continuation: Option<RegistryContinuationRequestV1>,
    relay: CustodyBumpRelayV1,
    activated: ActivatedExecutionReleaseSetViewV1<'_>,
    caller_authority: &AccountInfo<'_>,
    caller_program: &AccountInfo<'_>,
    replay: &AccountInfo<'_>,
) -> ProgramResult {
    let caller_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(request.release_set).map_err(|_| CustodySbfError::Release)?,
        request.market,
        registry_role(request.caller_role),
        request.context,
        request_digest,
    )
    .map_err(|_| CustodySbfError::CallerAuthority)?;
    // Reproduced from the bump the parent carried after the request, not
    // searched for; see `split_caller_authority_bump_v1`. A wrong bump
    // reproduces a different address and refuses at the equality below.
    let expected_caller = match relay.caller_authority {
        Some(bump) => {
            let bump_seed = [bump];
            let [domain, release, market, role, context, digest] = caller_seeds.as_slices();
            Pubkey::create_program_address(
                &[domain, release, market, role, context, digest, &bump_seed],
                caller_program.key,
            )
            .map_err(|_| CustodySbfError::CallerAuthority)?
        }
        None => Pubkey::find_program_address(&caller_seeds.as_slices(), caller_program.key).0,
    };
    if caller_authority.key != &expected_caller {
        return Err(CustodySbfError::CallerAuthority.into());
    }
    custody_cu_checkpoint!("cf-caller-authority");
    authenticate_calling_release(program_id, accounts, request, continuation, activated)?;
    custody_cu_checkpoint!("cf-calling-release");
    authenticate_replay_identity(program_id, replay, request, relay.replay)?;
    Ok(())
}

#[derive(Clone, Copy)]
enum AuthenticatedMarketAdmissionV1 {
    Live(AuthenticatedMarketV1),
    Premarket,
}

#[derive(Clone, Copy)]
struct AuthenticatedMarketV1 {
    state: CoreState,
}

/// Select the deliberately vacant future-Market path before ordinary Market
/// authentication, but only for an exact precommitted SeriesEscrow request.
///
/// System-owned/data-empty is the selection boundary.  Once selected, every
/// remaining mismatch is a refusal from this path; no partially authenticated
/// request can fall through to the live-Market verifier.
#[inline(never)]
fn try_authenticate_premarket_market(
    accounts: &[AccountInfo<'_>],
    request: CustodyRequestV1,
) -> Result<bool, ProgramError> {
    let market = account(accounts, CORE_MARKET)?;
    if !is_premarket_series_vacancy(market, request) {
        return Ok(false);
    }
    require_premarket_series_market(market, request)?;
    // The cache's own identity -- Registry ownership, the exact width, the body
    // naming this release set, and the address its carried bump reproduces --
    // is established once by the caller, from the same decode this path used to
    // make for itself, so nothing is left for this arm to authenticate.
    Ok(true)
}

fn is_premarket_series_vacancy(market: &AccountInfo<'_>, request: CustodyRequestV1) -> bool {
    classify_premarket_series_escrow_v1(request).is_some()
        && market.owner == &system_program::ID
        && market.data_len() == 0
}

fn require_premarket_series_market(
    market: &AccountInfo<'_>,
    request: CustodyRequestV1,
) -> ProgramResult {
    if market.key.to_bytes() != request.market
        || market.owner != &system_program::ID
        || market.data_len() != 0
        || market.is_signer
        || market.is_writable
        || market.executable
    {
        return Err(CustodySbfError::AccountFrame.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_market(
    accounts: &[AccountInfo<'_>],
    request: CustodyRequestV1,
    activated: ActivatedExecutionReleaseSetViewV1<'_>,
) -> Result<AuthenticatedMarketV1, ProgramError> {
    let market = account(accounts, CORE_MARKET)?;
    let registry = account(accounts, REGISTRY_PROGRAM)?;
    // ONE borrow, ONE decode. This used to call
    // `authenticate_activation_cache_bump_v1`, which borrows and decodes the
    // cache, and then borrow and decode the same immutable account AGAIN just
    // to read the Core role out of it -- re-taking the release-set equality the
    // first decode had already taken. `ActivatedExecutionReleaseSetViewV1::decode`
    // validates the complete five-role projection and all ten aliasing pairs,
    // twenty-five `decode_role` calls, every time. `require_cache_account` is
    // kept ahead of the borrow so owner and exact width are still required
    // before the first byte is read, and
    // `authenticate_activation_cache_identity_v1` takes them again over the
    // decoded view along with the release-set equality and the address.
    if market.data_len() != STATE_BYTES {
        return Err(CustodySbfError::Release.into());
    }
    let core_program = Pubkey::new_from_array(
        *activated
            .role(ExecutionRoleV1::Core)
            .map_err(|_| CustodySbfError::Release)?
            .release()
            .program()
            .as_bytes(),
    );
    if market.owner != &core_program {
        return Err(CustodySbfError::Release.into());
    }
    let bytes = market
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Release)?;
    let state = CoreState::decode(&bytes).map_err(|_| CustodySbfError::Release)?;
    if market.key.to_bytes() != request.market
        || state.identity.market_id.to_bytes() != request.market
        || state.identity.realm_id.to_bytes() != request.realm
        || state.identity.selected_release_set.to_bytes() != request.release_set
        || state.identity.registry_program.to_bytes() != registry.key.to_bytes()
        || state.identity.generation != request.semantic.generation
        || market_core_state_address_v2(state, &core_program)? != *market.key
    {
        return Err(CustodySbfError::Release.into());
    }
    Ok(AuthenticatedMarketV1 { state })
}

/// Reproduce one Realm record address at a recorded bump, or search for it.
///
/// The seeds are `[domain, REALM_SCHEMA_RELEASE_ID_V1, realm_digest]` under the
/// Registry. Only the digest varies, and the founding derived it from the very
/// bytes this reader just hashed.
fn registry_record_address_v1(
    domain: &[u8],
    realm_digest: &[u8; 32],
    registry: &Pubkey,
    recorded: Option<u8>,
) -> Result<Pubkey, ProgramError> {
    let base: [&[u8]; 3] = [domain, &REALM_SCHEMA_RELEASE_ID_V1, realm_digest];
    match recorded {
        Some(bump) => {
            let bump_seed = [bump];
            Pubkey::create_program_address(&[base[0], base[1], base[2], &bump_seed], registry)
                .map_err(|_| CustodySbfError::Realm.into())
        }
        None => Ok(Pubkey::find_program_address(&base, registry).0),
    }
}

/// Reproduce a Market's own Core state address, or search for it.
///
/// Nine seeds, all drawn from the Market identity, so all nine move with the
/// key draw -- and Trading, Claims and this program each derive the same
/// address on the same transaction. `CoreState` carries the bump the founding
/// derived, so the readers reproduce it.
///
/// The derivation IS the check: a wrong bump reproduces a different address,
/// which is compared against the account this frame was handed, and refuses.
/// Canonicality is enforced where the account is made -- Core creates market
/// states only at the canonical bump -- and not where it is read. A state
/// written before the bump tail existed carries none and is searched for
/// exactly as it used to be. See `StateBumpsV1`.
fn market_core_state_address_v2(
    state: CoreState,
    core_program: &Pubkey,
) -> Result<Pubkey, ProgramError> {
    let seeds = MarketCoreStateSeedsV2::new(state.identity);
    let base = seeds.as_slices();
    match state.bumps.market {
        Some(bump) => {
            let bump_seed = [bump];
            Pubkey::create_program_address(
                &[
                    base[0], base[1], base[2], base[3], base[4], base[5], base[6], base[7],
                    base[8], &bump_seed,
                ],
                core_program,
            )
            .map_err(|_| CustodySbfError::Release.into())
        }
        None => Ok(Pubkey::find_program_address(&base, core_program).0),
    }
}

#[inline(never)]
fn authenticate_calling_release(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CustodyRequestV1,
    continuation: Option<RegistryContinuationRequestV1>,
    activated: ActivatedExecutionReleaseSetViewV1<'_>,
) -> ProgramResult {
    if let Some(continuation) = continuation {
        return authenticate_registry_continuation(program_id, accounts, request, continuation);
    }
    let cache = account(accounts, ACTIVATION_CACHE)?;
    let caller_program = account(accounts, CALLER_PROGRAM)?;
    let caller_programdata = account(accounts, CALLER_PROGRAMDATA)?;
    let role = registry_role(request.caller_role);
    // Read the Registry-owned activation cache; never CPI back into the
    // Registry. Custody is reached at CPI depth three under a Registry
    // continuation, where the Registry sits at depth one, so an invocation from
    // here is reentrancy and the route could not execute at all. The cache
    // account is Registry-owned at a Registry-derived address and carries the
    // whole of what `Reauthenticate` would have returned.
    require_readonly_frame(cache, caller_program, caller_programdata)
        .map_err(CustodySbfError::from)?;
    let release = activated
        .role(role)
        .map_err(|_| CustodySbfError::Release)?
        .release();
    if release.program().to_bytes() != caller_program.key.to_bytes()
        || release.programdata() != caller_programdata.key.to_bytes()
        || release.program().to_bytes() != request.caller_program
    {
        return Err(CustodySbfError::Release.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_registry_continuation(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CustodyRequestV1,
    continuation: RegistryContinuationRequestV1,
) -> ProgramResult {
    if request.caller_role != CallerRoleV1::Core {
        return Err(CustodySbfError::Release.into());
    }
    let registry = account(accounts, REGISTRY_PROGRAM)?;
    let cache_account = account(accounts, ACTIVATION_CACHE)?;
    let caller_program = account(accounts, CALLER_PROGRAM)?;
    let caller_programdata = account(accounts, CALLER_PROGRAMDATA)?;
    let admission = accounts.last().ok_or(CustodySbfError::AccountFrame)?;
    require_continuation_role_profile(request.operation, continuation)?;
    if continuation.release_set_id().to_bytes() != request.release_set {
        return Err(CustodySbfError::Release.into());
    }
    let expected_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, request.release_set.as_slice()],
        registry.key,
    )
    .0;
    if expected_cache != *cache_account.key || cache_account.owner != registry.key {
        return Err(CustodySbfError::Release.into());
    }
    let cache_bytes = cache_account
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Release)?;
    if hash(&cache_bytes).to_bytes() != continuation.activation_cache_digest().to_bytes() {
        return Err(CustodySbfError::Release.into());
    }
    let cache = ActivatedExecutionReleaseSetViewV1::decode(&cache_bytes)
        .map_err(|_| CustodySbfError::Release)?;
    let core_release = cache
        .role(ExecutionRoleV1::Core)
        .map_err(|_| CustodySbfError::Release)?
        .release();
    let custody_release = cache
        .role(ExecutionRoleV1::Custody)
        .map_err(|_| CustodySbfError::Release)?
        .release();
    if cache
        .execution_release_set_id()
        .map_err(|_| CustodySbfError::Release)?
        .to_bytes()
        != request.release_set
        || core_release.program().to_bytes() != caller_program.key.to_bytes()
        || core_release.programdata() != caller_programdata.key.to_bytes()
        || custody_release.program().to_bytes() != program_id.to_bytes()
    {
        return Err(CustodySbfError::Release.into());
    }
    drop(cache_bytes);
    let batch = continuation
        .role_batch_request()
        .map_err(|_| CustodySbfError::Release)?;
    let batch_digest =
        ContentId::new(hash(&batch.to_bytes()).to_bytes()).map_err(|_| CustodySbfError::Release)?;
    let seeds = RegistryContinuationAdmissionSeedsV1::new(
        continuation,
        cache_account.key.to_bytes(),
        batch_digest,
    )
    .map_err(|_| CustodySbfError::Release)?;
    let release = seeds.release_set();
    let cache_key = seeds.activation_cache();
    let request_digest = seeds.batch_request_digest();
    let mask = seeds.role_mask();
    let role = seeds.continuation_role();
    let continuation_digest = seeds.continuation_digest();
    let expected = Pubkey::find_program_address(
        &[
            seeds.domain(),
            release.as_slice(),
            cache_key.as_slice(),
            request_digest.as_slice(),
            mask.as_slice(),
            role.as_slice(),
            continuation_digest.as_slice(),
        ],
        registry.key,
    )
    .0;
    if expected != *admission.key {
        return Err(CustodySbfError::Release.into());
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct RealmFacts {
    realm: RealmV1,
    profile: ExactTransferProfileV1,
}

#[inline(never)]
fn authenticate_realm(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CustodyRequestV1,
    market: CoreState,
    activated: ActivatedExecutionReleaseSetViewV1<'_>,
) -> Result<RealmFacts, ProgramError> {
    let registry = account(accounts, REGISTRY_PROGRAM)?;
    let realm_account = account(accounts, REALM)?;
    let staging = account(accounts, REALM_STAGING)?;
    if activated
        .execution_release_set_id()
        .map_err(|_| CustodySbfError::Release)?
        .as_bytes()
        != &request.release_set
        || activated
            .role(ExecutionRoleV1::Custody)
            .map_err(|_| CustodySbfError::Release)?
            .release()
            .program()
            .as_bytes()
            != &program_id.to_bytes()
    {
        return Err(CustodySbfError::Release.into());
    }

    if market.identity.realm_id.to_bytes() != request.realm
        || market.identity.registry_program.to_bytes() != registry.key.to_bytes()
        || realm_account.owner != registry.key
        || realm_account.data_len() != REALM_BYTES
        || !funded_rent_persists_v1(realm_account.lamports())
    {
        return Err(CustodySbfError::Realm.into());
    }
    let realm_data = realm_account
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Realm)?;
    let realm_digest = hash(&realm_data).to_bytes();
    // THE TWO MOST EXPENSIVE SEARCHES ON THE DIRECT ROUTE, and this function
    // runs once per Custody invocation -- twice per canonical trade, so four
    // searches. The founding authenticated this exact record pair and recorded
    // both bumps in the Market state, so they are reproduced here.
    //
    // The derivation IS the check: a wrong bump reproduces a different address,
    // which `require_realm_authority` compares against the account this frame
    // was handed, and refuses. Bumps a founding never recorded are `None` and
    // search, which is what every market opened before the tail existed does.
    let expected_realm = registry_record_address_v1(
        RAW_RECORD_PDA_SEED_V1,
        &realm_digest,
        registry.key,
        market.bumps.realm_raw_record,
    )?;
    let expected_staging = registry_record_address_v1(
        STAGING_CURSOR_PDA_SEED_V1,
        &realm_digest,
        registry.key,
        market.bumps.realm_staging_record,
    )?;
    require_realm_authority(RealmAuthorityObservation {
        registry: *registry.key,
        persisted_registry: Pubkey::new_from_array(market.identity.registry_program.to_bytes()),
        expected_realm,
        realm_key: *realm_account.key,
        realm_owner: *realm_account.owner,
        expected_staging,
        staging_key: *staging.key,
        staging_owner: *staging.owner,
        staging_data_len: staging.data_len(),
        realm_digest,
        expected_digest: request.realm,
    })?;
    let realm = RealmV1::decode(&realm_data).map_err(|_| CustodySbfError::Realm)?;
    let profile = collateral_profile(realm)?;
    if matches!(
        request.operation,
        OperationV1::OpenVault | OperationV1::Transfer | OperationV1::CloseVault
    ) && (request.mint != *realm.collateral_mint()
        || request.token_program != *realm.token_program()
        || request.token_program != profile.program_id())
    {
        return Err(CustodySbfError::Realm.into());
    }
    Ok(RealmFacts { realm, profile })
}

/// Authenticate the same finalized Realm authority as the live path without
/// borrowing Market-state bumps that cannot exist before the Market does.
///
/// The request's Realm content digest is signed by the current Trading caller.
/// Custody searches the Registry's canonical raw/staging coordinates from that
/// digest, requires the raw record to be finalized, and otherwise applies the
/// unchanged collateral profile checks.
#[inline(never)]
fn authenticate_premarket_realm(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CustodyRequestV1,
    activated: ActivatedExecutionReleaseSetViewV1<'_>,
) -> Result<RealmFacts, ProgramError> {
    let registry = account(accounts, REGISTRY_PROGRAM)?;
    let realm_account = account(accounts, REALM)?;
    let staging = account(accounts, REALM_STAGING)?;
    if activated
        .execution_release_set_id()
        .map_err(|_| CustodySbfError::Release)?
        .as_bytes()
        != &request.release_set
        || activated
            .role(ExecutionRoleV1::Custody)
            .map_err(|_| CustodySbfError::Release)?
            .release()
            .program()
            .as_bytes()
            != &program_id.to_bytes()
    {
        return Err(CustodySbfError::Release.into());
    }

    if realm_account.owner != registry.key
        || realm_account.data_len() != REALM_BYTES
        || !funded_rent_persists_v1(realm_account.lamports())
    {
        return Err(CustodySbfError::Realm.into());
    }
    let realm_data = realm_account
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Realm)?;
    let realm_digest = hash(&realm_data).to_bytes();
    let expected_realm =
        registry_record_address_v1(RAW_RECORD_PDA_SEED_V1, &request.realm, registry.key, None)?;
    let expected_staging = registry_record_address_v1(
        STAGING_CURSOR_PDA_SEED_V1,
        &request.realm,
        registry.key,
        None,
    )?;
    require_realm_authority(RealmAuthorityObservation {
        registry: *registry.key,
        persisted_registry: *registry.key,
        expected_realm,
        realm_key: *realm_account.key,
        realm_owner: *realm_account.owner,
        expected_staging,
        staging_key: *staging.key,
        staging_owner: *staging.owner,
        staging_data_len: staging.data_len(),
        realm_digest,
        expected_digest: request.realm,
    })?;
    let realm = RealmV1::decode(&realm_data).map_err(|_| CustodySbfError::Realm)?;
    let profile = collateral_profile(realm)?;
    if matches!(
        request.operation,
        OperationV1::OpenVault | OperationV1::Transfer | OperationV1::CloseVault
    ) && (request.mint != *realm.collateral_mint()
        || request.token_program != *realm.token_program()
        || request.token_program != profile.program_id())
    {
        return Err(CustodySbfError::Realm.into());
    }
    Ok(RealmFacts { realm, profile })
}

#[derive(Clone, Copy)]
struct RealmAuthorityObservation {
    registry: Pubkey,
    persisted_registry: Pubkey,
    expected_realm: Pubkey,
    realm_key: Pubkey,
    realm_owner: Pubkey,
    expected_staging: Pubkey,
    staging_key: Pubkey,
    staging_owner: Pubkey,
    staging_data_len: usize,
    realm_digest: [u8; 32],
    expected_digest: [u8; 32],
}

fn require_realm_authority(observed: RealmAuthorityObservation) -> ProgramResult {
    if observed.registry != observed.persisted_registry
        || observed.realm_key != observed.expected_realm
        || observed.realm_owner != observed.registry
        || observed.staging_key != observed.expected_staging
        || observed.staging_owner != system_program::ID
        || observed.staging_data_len != 0
        || observed.realm_digest != observed.expected_digest
    {
        return Err(CustodySbfError::Realm.into());
    }
    Ok(())
}

fn collateral_profile(realm: RealmV1) -> Result<ExactTransferProfileV1, ProgramError> {
    for release in PRODUCTION_ADAPTER_RELEASES {
        if hash(&release.to_bytes()).as_ref() == realm.collateral_adapter_release_id() {
            return Ok(release.profile());
        }
    }
    Err(CustodySbfError::Realm.into())
}

/// Reproduce this program's own replay coordinate at the parent's mined bump.
///
/// The seeds run through the Market, which moves with the participant keys, so
/// this search is one of the ten VARIANCE censused. It cannot be stored:
/// `CUSTODY_REPLAY_BYTES_V1` is exactly packed and Lean-emitted, and the record
/// whose bump it would carry is the replay itself, so a byte for it would have
/// to be written before the account exists. The parent mines it instead and
/// relays it after the request. A wrong byte reproduces a different address and
/// refuses at the equality below, unchanged.
fn authenticate_replay_identity(
    program_id: &Pubkey,
    replay: &AccountInfo<'_>,
    request: CustodyRequestV1,
    hint: Option<u8>,
) -> ProgramResult {
    let replay_seeds = CustodyReplaySeedsV1::from_request(request);
    let expected = match hint {
        Some(bump) => {
            let bump_seed = [bump];
            let [domain, market, release, role, context] = replay_seeds.as_slices();
            Pubkey::create_program_address(
                &[domain, market, release, role, context, &bump_seed],
                program_id,
            )
            .map_err(|_| CustodySbfError::Replay)?
        }
        None => Pubkey::find_program_address(&replay_seeds.as_slices(), program_id).0,
    };
    if replay.key != &expected {
        return Err(CustodySbfError::Replay.into());
    }
    match request.operation {
        OperationV1::InitializeReplay => {
            if replay.owner != &system_program::ID || replay.data_len() != 0 {
                return Err(CustodySbfError::Replay.into());
            }
        }
        OperationV1::OpenVault
        | OperationV1::Transfer
        | OperationV1::CloseVault
        | OperationV1::CloseReplay => {
            if replay.owner != program_id || replay.data_len() != CUSTODY_REPLAY_BYTES_V1 {
                return Err(CustodySbfError::Replay.into());
            }
        }
    }
    Ok(())
}

#[inline(never)]
fn initialize_replay(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CustodyRequestV1,
    request_digest: [u8; 32],
) -> ProgramResult {
    let replay = account(accounts, REPLAY)?;
    let payer = account(accounts, 9)?;
    let system = account(accounts, 10)?;
    let rent_account = account(accounts, 11)?;
    let rent_refund = account(accounts, 12)?;
    if system.key != &system_program::ID
        || rent_account.key != &sysvar::rent::ID
        || payer.key.to_bytes() != request.payer
        || rent_refund.key.to_bytes() != request.rent_refund
        || rent_refund.key == payer.key
        || rent_refund.key == replay.key
    {
        return Err(CustodySbfError::AccountFrame.into());
    }
    let rent = Rent::from_account_info(rent_account).map_err(|_| CustodySbfError::Create)?;
    let exact_rent = rent.minimum_balance(CUSTODY_REPLAY_BYTES_V1);
    if exact_rent != request.rent_lamports {
        return Err(CustodySbfError::Create.into());
    }
    let replay_seeds = CustodyReplaySeedsV1::from_request(request);
    let bump = Pubkey::find_program_address(&replay_seeds.as_slices(), program_id).1;
    let bump_seed = [bump];
    let [domain, market, release, role, context] = replay_seeds.as_slices();
    let signer_seeds = &[domain, market, release, role, context, &bump_seed];
    let observed_lamports = replay.lamports();
    let normalization =
        replay_rent_normalization(observed_lamports, exact_rent, rent_refund.lamports())?;
    if normalization.excess != 0 {
        invoke_signed(
            &transfer(replay.key, rent_refund.key, normalization.excess),
            &[replay.clone(), rent_refund.clone(), system.clone()],
            &[signer_seeds],
        )
        .map_err(|_| CustodySbfError::Create)?;
    } else if normalization.shortfall != 0 {
        invoke(
            &transfer(payer.key, replay.key, normalization.shortfall),
            &[payer.clone(), replay.clone(), system.clone()],
        )
        .map_err(|_| CustodySbfError::Create)?;
    }
    invoke_signed(
        &allocate(
            replay.key,
            u64::try_from(CUSTODY_REPLAY_BYTES_V1).map_err(|_| CustodySbfError::Create)?,
        ),
        &[replay.clone(), system.clone()],
        &[signer_seeds],
    )
    .map_err(|_| CustodySbfError::Create)?;
    invoke_signed(
        &assign(replay.key, program_id),
        &[replay.clone(), system.clone()],
        &[signer_seeds],
    )
    .map_err(|_| CustodySbfError::Create)?;
    if replay.owner != program_id
        || replay.data_len() != CUSTODY_REPLAY_BYTES_V1
        || replay.lamports() != exact_rent
    {
        return Err(CustodySbfError::Create.into());
    }
    let poststate = poststate_commitment(PoststateProjection {
        request_digest,
        source: replay.key.to_bytes(),
        destination: replay.key.to_bytes(),
        source_before: 0,
        source_after: 0,
        destination_before: 0,
        destination_after: 0,
        rent_lamports: exact_rent,
    });
    let replay_state = CustodyReplayV1::initialize(request, request_digest, poststate)
        .map_err(|_| CustodySbfError::Replay)?;
    commit_replay_and_receipt(
        replay,
        request,
        request_digest,
        replay_state,
        zero_evidence(poststate),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReplayRentNormalizationV1 {
    shortfall: u64,
    excess: u64,
}

fn replay_rent_normalization(
    observed_lamports: u64,
    exact_rent: u64,
    refund_lamports: u64,
) -> Result<ReplayRentNormalizationV1, ProgramError> {
    if observed_lamports > exact_rent {
        let excess = observed_lamports
            .checked_sub(exact_rent)
            .ok_or(CustodySbfError::Create)?;
        refund_lamports
            .checked_add(excess)
            .ok_or(CustodySbfError::Create)?;
        Ok(ReplayRentNormalizationV1 {
            shortfall: 0,
            excess,
        })
    } else {
        Ok(ReplayRentNormalizationV1 {
            shortfall: exact_rent
                .checked_sub(observed_lamports)
                .ok_or(CustodySbfError::Create)?,
            excess: 0,
        })
    }
}

#[inline(never)]
fn open_vault(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CustodyRequestV1,
    request_digest: [u8; 32],
    realm: RealmFacts,
) -> ProgramResult {
    let mint = account(accounts, 9)?;
    let vault = account(accounts, 10)?;
    let authority = account(accounts, 11)?;
    let token_program = account(accounts, 12)?;
    let payer = account(accounts, 13)?;
    let system = account(accounts, 14)?;
    let rent_account = account(accounts, 15)?;
    validate_token_program_and_mint(mint, token_program, request, realm)?;
    let _authority_bump = validate_custody_authority(program_id, authority, request, None)?;
    validate_vault_key(program_id, vault, request, false)?;
    if vault.owner != &system_program::ID
        || vault.lamports() != 0
        || vault.data_len() != 0
        || payer.key.to_bytes() != request.payer
        || system.key != &system_program::ID
        || rent_account.key != &sysvar::rent::ID
    {
        return Err(CustodySbfError::AccountFrame.into());
    }
    let rent = Rent::from_account_info(rent_account).map_err(|_| CustodySbfError::Create)?;
    let exact_rent = rent.minimum_balance(dclutch_custody::token_svm::ACCOUNT_BYTES);
    if request.rent_lamports != exact_rent {
        return Err(CustodySbfError::Create.into());
    }
    create_vault(
        program_id,
        payer,
        vault,
        system,
        token_program,
        request,
        exact_rent,
    )?;
    initialize_vault(vault, mint, authority, token_program, request)?;
    let token = read_custody_account(vault, token_program, mint, authority, realm.profile)?;
    if token.amount != 0 || vault.lamports() != exact_rent {
        return Err(CustodySbfError::Postcondition.into());
    }
    let replay = read_replay(account(accounts, REPLAY)?)?;
    let poststate = poststate_commitment(PoststateProjection {
        request_digest,
        source: vault.key.to_bytes(),
        destination: vault.key.to_bytes(),
        source_before: 0,
        source_after: 0,
        destination_before: 0,
        destination_after: 0,
        rent_lamports: exact_rent,
    });
    let next = replay
        .advance(request, request_digest, poststate)
        .map_err(|_| CustodySbfError::Replay)?;
    commit_replay_and_receipt(
        account(accounts, REPLAY)?,
        request,
        request_digest,
        next,
        zero_evidence(poststate),
    )
}

#[inline(never)]
fn execute_transfer(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CustodyRequestV1,
    request_digest: [u8; 32],
    realm: RealmFacts,
    relay: CustodyBumpRelayV1,
) -> ProgramResult {
    // External debits must use the distinct V2 wire so an apparently correct
    // balance delta cannot retain hidden delegated spending authority.
    if request.source_compartment == CompartmentV1::External {
        return Err(CustodySbfError::Instruction.into());
    }
    let mint = account(accounts, 9)?;
    let source = account(accounts, 10)?;
    let destination = account(accounts, 11)?;
    let authority = account(accounts, 12)?;
    let token_program = account(accounts, 13)?;
    custody_cu_checkpoint!("cu-transfer-frame");
    validate_token_program_and_mint(mint, token_program, request, realm)?;
    let authority_bump =
        validate_custody_authority(program_id, authority, request, relay.transfer_authority)?;
    if source.key.to_bytes() != request.source
        || destination.key.to_bytes() != request.destination
        || source.owner != token_program.key
        || destination.owner != token_program.key
    {
        return Err(CustodySbfError::AccountFrame.into());
    }
    if request.source_compartment != CompartmentV1::External {
        validate_vault_key(program_id, source, request, true)?;
    }
    if request.destination_compartment != CompartmentV1::External {
        validate_vault_key(program_id, destination, request, false)?;
    }
    let transfer_accounts = TransferAccounts {
        source,
        destination,
        mint,
        authority,
        token_program,
    };
    custody_cu_checkpoint!("cu-transfer-validated");
    let before = authenticate_transfer_accounts(transfer_accounts, request, realm.profile, true)?;
    custody_cu_checkpoint!("cu-prestate");
    invoke_exact_transfer(transfer_accounts, request, before.decimals, authority_bump)?;
    custody_cu_checkpoint!("cu-token-cpi");
    let after = authenticate_transfer_accounts(transfer_accounts, request, realm.profile, false)?;
    custody_cu_checkpoint!("cu-poststate");
    if before
        .source
        .checked_sub(request.amount)
        .ok_or(CustodySbfError::Postcondition)?
        != after.source
        || before
            .destination
            .checked_add(request.amount)
            .ok_or(CustodySbfError::Postcondition)?
            != after.destination
    {
        return Err(CustodySbfError::Postcondition.into());
    }
    let evidence = ReceiptEvidenceV1 {
        source_before: before.source,
        source_after: after.source,
        destination_before: before.destination,
        destination_after: after.destination,
        poststate_commitment: poststate_commitment(PoststateProjection {
            request_digest,
            source: source.key.to_bytes(),
            destination: destination.key.to_bytes(),
            source_before: before.source,
            source_after: after.source,
            destination_before: before.destination,
            destination_after: after.destination,
            rent_lamports: 0,
        }),
        replay_state_digest: [0; 32],
    };
    let replay = read_replay(account(accounts, REPLAY)?)?;
    let next = replay
        .advance(request, request_digest, evidence.poststate_commitment)
        .map_err(|_| CustodySbfError::Replay)?;
    commit_replay_and_receipt(
        account(accounts, REPLAY)?,
        request,
        request_digest,
        next,
        evidence,
    )
}

#[inline(never)]
fn close_vault(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CustodyRequestV1,
    request_digest: [u8; 32],
    realm: RealmFacts,
) -> ProgramResult {
    let mint = account(accounts, 9)?;
    let vault = account(accounts, 10)?;
    let authority = account(accounts, 11)?;
    let token_program = account(accounts, 12)?;
    let rent_refund = account(accounts, 13)?;
    validate_token_program_and_mint(mint, token_program, request, realm)?;
    let authority_bump = validate_custody_authority(program_id, authority, request, None)?;
    validate_vault_key(program_id, vault, request, true)?;
    if rent_refund.key.to_bytes() != request.rent_refund {
        return Err(CustodySbfError::AccountFrame.into());
    }
    let token = read_custody_account(vault, token_program, mint, authority, realm.profile)?;
    let vault_lamports = vault.lamports();
    let refund_before = rent_refund.lamports();
    if token.amount != 0 || vault_lamports != request.rent_lamports {
        return Err(CustodySbfError::TokenState.into());
    }
    refund_before
        .checked_add(vault_lamports)
        .ok_or(CustodySbfError::Postcondition)?;
    invoke_close(
        vault,
        rent_refund,
        authority,
        token_program,
        request,
        authority_bump,
    )?;
    if vault.lamports() != 0
        || rent_refund.lamports()
            != refund_before
                .checked_add(vault_lamports)
                .ok_or(CustodySbfError::Postcondition)?
    {
        return Err(CustodySbfError::Postcondition.into());
    }
    let poststate = poststate_commitment(PoststateProjection {
        request_digest,
        source: vault.key.to_bytes(),
        destination: rent_refund.key.to_bytes(),
        source_before: 0,
        source_after: 0,
        destination_before: 0,
        destination_after: 0,
        rent_lamports: vault_lamports,
    });
    let replay = read_replay(account(accounts, REPLAY)?)?;
    let next = replay
        .advance(request, request_digest, poststate)
        .map_err(|_| CustodySbfError::Replay)?;
    commit_replay_and_receipt(
        account(accounts, REPLAY)?,
        request,
        request_digest,
        next,
        zero_evidence(poststate),
    )
}

#[inline(never)]
fn close_replay(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CustodyRequestV1,
    request_digest: [u8; 32],
) -> ProgramResult {
    let replay = account(accounts, REPLAY)?;
    let rent_refund = account(accounts, 9)?;
    if rent_refund.key == replay.key || rent_refund.key.to_bytes() != request.rent_refund {
        return Err(CustodySbfError::AccountFrame.into());
    }
    let replay_lamports = replay.lamports();
    let refund_before = rent_refund.lamports();
    let refund_after = refund_before
        .checked_add(replay_lamports)
        .ok_or(CustodySbfError::Postcondition)?;
    if replay.owner != program_id
        || replay.data_len() != CUSTODY_REPLAY_BYTES_V1
        || replay_lamports != request.rent_lamports
    {
        return Err(CustodySbfError::Replay.into());
    }
    let poststate = poststate_commitment(PoststateProjection {
        request_digest,
        source: replay.key.to_bytes(),
        destination: rent_refund.key.to_bytes(),
        source_before: 0,
        source_after: 0,
        destination_before: 0,
        destination_after: 0,
        rent_lamports: replay_lamports,
    });
    let replay_state = read_replay(replay)?;
    replay_state
        .advance(request, request_digest, poststate)
        .map_err(|_| CustodySbfError::Replay)?;
    let evidence = ReceiptEvidenceV1 {
        replay_state_digest: hash(&[]).to_bytes(),
        ..zero_evidence(poststate)
    };
    let receipt = CustodyReceiptV1::new(request, request_digest, evidence)
        .map_err(|_| CustodySbfError::Postcondition)?;
    let receipt_bytes = receipt
        .to_bytes()
        .map_err(|_| CustodySbfError::Postcondition)?;
    if receipt_bytes.len() != CUSTODY_RECEIPT_BYTES_V1 {
        return Err(CustodySbfError::Postcondition.into());
    }

    {
        let mut replay_data = replay
            .try_borrow_mut_data()
            .map_err(|_| CustodySbfError::Commit)?;
        if replay_data.len() != CUSTODY_REPLAY_BYTES_V1 {
            return Err(CustodySbfError::Commit.into());
        }
        replay_data.fill(0);
    }
    {
        let mut replay_balance = replay
            .try_borrow_mut_lamports()
            .map_err(|_| CustodySbfError::Commit)?;
        let mut refund_balance = rent_refund
            .try_borrow_mut_lamports()
            .map_err(|_| CustodySbfError::Commit)?;
        **replay_balance = 0;
        **refund_balance = refund_after;
    }
    replay.resize(0).map_err(|_| CustodySbfError::Commit)?;
    replay.assign(&system_program::ID);
    if replay.lamports() != 0
        || replay.data_len() != 0
        || replay.owner != &system_program::ID
        || rent_refund.lamports() != refund_after
    {
        return Err(CustodySbfError::Postcondition.into());
    }
    set_return_data(&receipt_bytes);
    Ok(())
}

fn require_account_count(
    accounts: &[AccountInfo<'_>],
    operation: OperationV1,
    continuation: bool,
) -> ProgramResult {
    let spec = CustodyFrameSpecV1::new(operation);
    let base = usize::from(spec.account_count());
    let expected = base
        .checked_add(usize::from(continuation))
        .ok_or(CustodySbfError::AccountFrame)?;
    if accounts.len() != expected {
        return Err(CustodySbfError::AccountFrame.into());
    }
    if continuation && continuation_roles(operation).is_none() {
        return Err(CustodySbfError::AccountFrame.into());
    }
    for (index, observed) in accounts.iter().take(base).enumerate() {
        let coordinate = u16::try_from(index).map_err(|_| CustodySbfError::AccountFrame)?;
        let expected = spec
            .account(coordinate)
            .map_err(|_| CustodySbfError::AccountFrame)?
            .privileges();
        if observed.is_signer != expected.signer()
            || observed.is_writable != expected.writable()
            || observed.executable != expected.executable()
        {
            return Err(CustodySbfError::AccountFrame.into());
        }
    }
    if continuation {
        let admission = accounts.get(base).ok_or(CustodySbfError::AccountFrame)?;
        if !admission.is_signer
            || admission.is_writable
            || admission.executable
            || admission.owner != &system_program::ID
            || !admission.data_is_empty()
            || admission.lamports() != 0
            || accounts
                .get(..base)
                .is_some_and(|prefix| prefix.iter().any(|account| account.key == admission.key))
        {
            return Err(CustodySbfError::AccountFrame.into());
        }
    }
    Ok(())
}

fn continuation_roles(operation: OperationV1) -> Option<&'static [ExecutionRoleV1]> {
    match operation {
        OperationV1::InitializeReplay | OperationV1::OpenVault => {
            Some(&[ExecutionRoleV1::Core, ExecutionRoleV1::Custody])
        }
        OperationV1::CloseVault | OperationV1::CloseReplay => Some(&[
            ExecutionRoleV1::Core,
            ExecutionRoleV1::Claims,
            ExecutionRoleV1::Resolution,
            ExecutionRoleV1::Custody,
        ]),
        OperationV1::Transfer => None,
    }
}

fn require_continuation_role_profile(
    operation: OperationV1,
    continuation: RegistryContinuationRequestV1,
) -> ProgramResult {
    let roles = continuation_roles(operation).ok_or(CustodySbfError::AccountFrame)?;
    if continuation.continuation_role() != ExecutionRoleV1::Core
        || usize::from(continuation.role_count()) != roles.len()
        || roles
            .iter()
            .enumerate()
            .any(|(index, role)| continuation.role(index) != Some(*role))
    {
        return Err(CustodySbfError::Release.into());
    }
    Ok(())
}

fn registry_role(role: CallerRoleV1) -> ExecutionRoleV1 {
    role
}

fn account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    index: usize,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| CustodySbfError::AccountFrame.into())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthenticatedCustodyAuthorityBumpV1(u8);

/// Reproduce the transfer authority at the parent's mined bump, or search.
///
/// Its seeds are `[domain, market, release_set]` and the Market moves with the
/// participant keys, so this is another of the ten. The bump it returns is the
/// one this program then SIGNS the token transfer with, which is exactly why a
/// hint here is safe: a non-canonical byte produces an address that is not the
/// authority account the frame supplied, the equality below refuses, and no
/// signature is ever attempted with it.
fn validate_custody_authority(
    program_id: &Pubkey,
    authority: &AccountInfo<'_>,
    request: CustodyRequestV1,
    hint: Option<u8>,
) -> Result<AuthenticatedCustodyAuthorityBumpV1, ProgramError> {
    let authority_seeds = CustodyAuthoritySeedsV1::from_request(request);
    let (expected, bump) = match hint {
        Some(bump) => {
            let bump_seed = [bump];
            let [domain, market, release] = authority_seeds.as_slices();
            (
                Pubkey::create_program_address(&[domain, market, release, &bump_seed], program_id)
                    .map_err(|_| CustodySbfError::TokenState)?,
                bump,
            )
        }
        None => Pubkey::find_program_address(&authority_seeds.as_slices(), program_id),
    };
    if authority.key != &expected {
        return Err(CustodySbfError::TokenState.into());
    }
    Ok(AuthenticatedCustodyAuthorityBumpV1(bump))
}

fn validate_vault_key(
    program_id: &Pubkey,
    vault: &AccountInfo<'_>,
    request: CustodyRequestV1,
    source: bool,
) -> ProgramResult {
    let vault_seeds = CustodyVaultSeedsV1::from_request(request, source);
    let expected = Pubkey::find_program_address(&vault_seeds.as_slices(), program_id).0;
    if vault.key != &expected {
        return Err(CustodySbfError::TokenState.into());
    }
    Ok(())
}

fn validate_token_program_and_mint(
    mint: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    request: CustodyRequestV1,
    facts: RealmFacts,
) -> ProgramResult {
    if token_program.key.to_bytes() != request.token_program
        || mint.key.to_bytes() != request.mint
        || mint.owner != token_program.key
    {
        return Err(CustodySbfError::TokenState.into());
    }
    let data = mint
        .try_borrow_data()
        .map_err(|_| CustodySbfError::TokenState)?;
    let mint_state = facts
        .profile
        .check_mint(request.token_program, &data)
        .map_err(|_| CustodySbfError::TokenState)?;
    if (facts.realm.mint_authority_policy() == MintAuthorityPolicy::RequireAbsent
        && !matches!(mint_state.mint_authority, COption::None))
        || (facts.realm.freeze_authority_policy() == FreezeAuthorityPolicy::RequireAbsent
            && !matches!(mint_state.freeze_authority, COption::None))
    {
        return Err(CustodySbfError::TokenState.into());
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct TransferBalances {
    source: u64,
    destination: u64,
    decimals: u8,
}

#[derive(Clone, Copy)]
struct TransferAccounts<'a, 'info> {
    source: &'a AccountInfo<'info>,
    destination: &'a AccountInfo<'info>,
    mint: &'a AccountInfo<'info>,
    authority: &'a AccountInfo<'info>,
    token_program: &'a AccountInfo<'info>,
}

fn authenticate_transfer_accounts(
    accounts: TransferAccounts<'_, '_>,
    request: CustodyRequestV1,
    profile: ExactTransferProfileV1,
    require_authority: bool,
) -> Result<TransferBalances, ProgramError> {
    let mint_data = accounts
        .mint
        .try_borrow_data()
        .map_err(|_| CustodySbfError::TokenState)?;
    let source_data = accounts
        .source
        .try_borrow_data()
        .map_err(|_| CustodySbfError::TokenState)?;
    let destination_data = accounts
        .destination
        .try_borrow_data()
        .map_err(|_| CustodySbfError::TokenState)?;
    let mint_state = profile
        .check_mint(request.token_program, &mint_data)
        .map_err(|_| CustodySbfError::TokenState)?;
    let source_state = profile
        .check_transfer_account(request.token_program, &source_data)
        .map_err(|_| CustodySbfError::TokenState)?;
    let destination_state = profile
        .check_transfer_account(request.token_program, &destination_data)
        .map_err(|_| CustodySbfError::TokenState)?;
    if source_state.mint != request.mint || destination_state.mint != request.mint {
        return Err(CustodySbfError::TokenState.into());
    }
    let authority_role = if require_authority {
        Some(
            profile
                .check_transfer(ExactTransferInput {
                    program_id: request.token_program,
                    mint_address: request.mint,
                    mint_data: &mint_data,
                    source_data: &source_data,
                    destination_data: &destination_data,
                    authority: accounts.authority.key.to_bytes(),
                    amount: request.amount,
                    decimals: mint_state.decimals,
                })
                .map_err(|_| CustodySbfError::TokenState)?
                .authority_role(),
        )
    } else {
        None
    };
    if request.source_compartment == CompartmentV1::External {
        if (require_authority && authority_role != Some(AuthorityRole::Delegate))
            || source_state.owner == accounts.authority.key.to_bytes()
            || source_state.owner != request.semantic.source_owner
        {
            return Err(CustodySbfError::TokenState.into());
        }
    } else {
        profile
            .check_custody_account(
                request.token_program,
                &source_data,
                request.mint,
                accounts.authority.key.to_bytes(),
            )
            .map_err(|_| CustodySbfError::TokenState)?;
    }
    if request.destination_compartment == CompartmentV1::External {
        if destination_state.owner == accounts.authority.key.to_bytes()
            || destination_state.owner != request.semantic.destination_owner
        {
            return Err(CustodySbfError::TokenState.into());
        }
    } else {
        profile
            .check_custody_account(
                request.token_program,
                &destination_data,
                request.mint,
                accounts.authority.key.to_bytes(),
            )
            .map_err(|_| CustodySbfError::TokenState)?;
    }
    Ok(TransferBalances {
        source: source_state.amount,
        destination: destination_state.amount,
        decimals: mint_state.decimals,
    })
}

fn read_custody_account(
    account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    authority: &AccountInfo<'_>,
    profile: ExactTransferProfileV1,
) -> Result<dclutch_custody::token_svm::TokenAccount, ProgramError> {
    if account.owner != token_program.key {
        return Err(CustodySbfError::TokenState.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| CustodySbfError::TokenState)?;
    profile
        .check_custody_account(
            token_program.key.to_bytes(),
            &data,
            mint.key.to_bytes(),
            authority.key.to_bytes(),
        )
        .map_err(|_| CustodySbfError::TokenState.into())
}

fn create_vault<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    vault: &AccountInfo<'a>,
    system: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    request: CustodyRequestV1,
    rent_lamports: u64,
) -> ProgramResult {
    let instruction = create_account(
        payer.key,
        vault.key,
        rent_lamports,
        u64::try_from(dclutch_custody::token_svm::ACCOUNT_BYTES).map_err(|_| CustodySbfError::Create)?,
        token_program.key,
    );
    let vault_seeds = CustodyVaultSeedsV1::from_request(request, false);
    let bump = Pubkey::find_program_address(&vault_seeds.as_slices(), program_id).1;
    let bump_seed = [bump];
    let [domain, market, release, context, compartment] = vault_seeds.as_slices();
    invoke_signed(
        &instruction,
        &[payer.clone(), vault.clone(), system.clone()],
        &[&[domain, market, release, context, compartment, &bump_seed]],
    )
    .map_err(|_| CustodySbfError::Create.into())
}

fn initialize_vault<'a>(
    vault: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    request: CustodyRequestV1,
) -> ProgramResult {
    let specification = initialize_account3(
        request.token_program,
        request.destination,
        request.mint,
        authority.key.to_bytes(),
    )
    .map_err(|_| CustodySbfError::TokenState)?;
    let instruction = token_instruction(&specification);
    invoke(
        &instruction,
        &[vault.clone(), mint.clone(), token_program.clone()],
    )
    .map_err(|_| CustodySbfError::TokenCpi.into())
}

fn invoke_exact_transfer(
    accounts: TransferAccounts<'_, '_>,
    request: CustodyRequestV1,
    decimals: u8,
    authority_bump: AuthenticatedCustodyAuthorityBumpV1,
) -> ProgramResult {
    let specification = transfer_checked(
        request.token_program,
        request.source,
        request.mint,
        request.destination,
        accounts.authority.key.to_bytes(),
        request.amount,
        decimals,
    )
    .map_err(|_| CustodySbfError::TokenState)?;
    let instruction = token_instruction(&specification);
    let authority_seeds = CustodyAuthoritySeedsV1::from_request(request);
    let bump_seed = [authority_bump.0];
    let [domain, market, release] = authority_seeds.as_slices();
    invoke_signed(
        &instruction,
        &[
            accounts.source.clone(),
            accounts.mint.clone(),
            accounts.destination.clone(),
            accounts.authority.clone(),
            accounts.token_program.clone(),
        ],
        &[&[domain, market, release, &bump_seed]],
    )
    .map_err(|_| CustodySbfError::TokenCpi.into())
}

fn invoke_close<'a>(
    vault: &AccountInfo<'a>,
    rent_refund: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    request: CustodyRequestV1,
    authority_bump: AuthenticatedCustodyAuthorityBumpV1,
) -> ProgramResult {
    let specification = close_account(
        request.token_program,
        request.source,
        request.rent_refund,
        authority.key.to_bytes(),
    )
    .map_err(|_| CustodySbfError::TokenState)?;
    let instruction = token_instruction(&specification);
    let authority_seeds = CustodyAuthoritySeedsV1::from_request(request);
    let bump_seed = [authority_bump.0];
    let [domain, market, release] = authority_seeds.as_slices();
    invoke_signed(
        &instruction,
        &[
            vault.clone(),
            rent_refund.clone(),
            authority.clone(),
            token_program.clone(),
        ],
        &[&[domain, market, release, &bump_seed]],
    )
    .map_err(|_| CustodySbfError::TokenCpi.into())
}

fn token_instruction<const ACCOUNTS: usize, const DATA: usize>(
    specification: &dclutch_custody::token_svm::InstructionSpec<ACCOUNTS, DATA>,
) -> Instruction {
    let mut accounts = Vec::with_capacity(ACCOUNTS);
    for role in specification.accounts() {
        let address = Pubkey::new_from_array(*role.address());
        accounts.push(if role.is_writable() {
            AccountMeta::new(address, role.is_signer())
        } else {
            AccountMeta::new_readonly(address, role.is_signer())
        });
    }
    Instruction {
        program_id: Pubkey::new_from_array(*specification.program_id()),
        accounts,
        data: specification.data().to_vec(),
    }
}

fn read_replay(replay: &AccountInfo<'_>) -> Result<CustodyReplayV1, ProgramError> {
    let data = replay
        .try_borrow_data()
        .map_err(|_| CustodySbfError::Replay)?;
    CustodyReplayV1::decode(&data).map_err(|_| CustodySbfError::Replay.into())
}

fn zero_evidence(poststate_commitment: [u8; 32]) -> ReceiptEvidenceV1 {
    ReceiptEvidenceV1 {
        source_before: 0,
        source_after: 0,
        destination_before: 0,
        destination_after: 0,
        poststate_commitment,
        replay_state_digest: [0; 32],
    }
}

#[inline(never)]
fn commit_replay_and_receipt(
    replay: &AccountInfo<'_>,
    request: CustodyRequestV1,
    request_digest: [u8; 32],
    replay_state: CustodyReplayV1,
    mut evidence: ReceiptEvidenceV1,
) -> ProgramResult {
    let replay_bytes = replay_state
        .to_bytes()
        .map_err(|_| CustodySbfError::Replay)?;
    evidence.replay_state_digest = hash(&replay_bytes).to_bytes();
    let receipt = CustodyReceiptV1::new(request, request_digest, evidence)
        .map_err(|_| CustodySbfError::Postcondition)?;
    let receipt_bytes = receipt
        .to_bytes()
        .map_err(|_| CustodySbfError::Postcondition)?;
    if receipt_bytes.len() != CUSTODY_RECEIPT_BYTES_V1 {
        return Err(CustodySbfError::Postcondition.into());
    }
    let mut data = replay
        .try_borrow_mut_data()
        .map_err(|_| CustodySbfError::Commit)?;
    if data.len() != CUSTODY_REPLAY_BYTES_V1 {
        return Err(CustodySbfError::Commit.into());
    }
    data.copy_from_slice(&replay_bytes);
    drop(data);
    set_return_data(&receipt_bytes);
    Ok(())
}

#[derive(Clone, Copy)]
struct PoststateProjection {
    request_digest: [u8; 32],
    source: [u8; 32],
    destination: [u8; 32],
    source_before: u64,
    source_after: u64,
    destination_before: u64,
    destination_after: u64,
    rent_lamports: u64,
}

fn poststate_commitment(projection: PoststateProjection) -> [u8; 32] {
    hashv(&[
        dclutch_custody::CUSTODY_POSTSTATE_DOMAIN_V1,
        &projection.request_digest,
        &projection.source,
        &projection.destination,
        &projection.source_before.to_le_bytes(),
        &projection.source_after.to_le_bytes(),
        &projection.destination_before.to_le_bytes(),
        &projection.destination_after.to_le_bytes(),
        &projection.rent_lamports.to_le_bytes(),
    ])
    .to_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE INVARIANT THE CALLER-AUTHORITY BUMP CARRY RESTS ON.
    ///
    /// `CallerAuthoritySeedsV1`'s last seed is `hash(request_bytes)`, and
    /// `request_bytes` is what `split_registry_continuation` hands back -- the
    /// request, never the whole instruction data. That is the only reason the
    /// parent can append its bump after the request: a bump inside the hashed
    /// region would change the digest, which changes the address, which changes
    /// the bump, and the carrier would have no fixed point.
    ///
    /// **If you are here because this row went red:** something widened the
    /// digest to cover more than the request. That is a reasonable-looking
    /// hardening and it silently breaks the carry -- what fails afterwards is
    /// `authenticate_common_frame`, reproducing an address nobody signed with,
    /// for reasons no refusal code explains. Either keep the digest over the
    /// request, or remove `split_caller_authority_bump_v1` and the push in
    /// `trading-sbf/src/custody_composition_v3.rs` in the same change.
    #[test]
    fn the_caller_authority_digest_covers_the_request_prefix_only() {
        let request = alloc::vec![0x5a_u8; dclutch_custody::CUSTODY_REQUEST_BYTES_V1];
        let bare = hash(&request).to_bytes();

        let mut carried = request.clone();
        carried.push(0xfd);
        let (stripped, relay) = split_caller_authority_bump_v1(&carried);
        assert_eq!(relay.caller_authority, Some(0xfd));
        assert_eq!(stripped, &request[..]);
        let (digest_input, continuation) =
            split_registry_continuation(stripped).expect("a bare V1 request");
        assert!(continuation.is_none());
        assert_eq!(
            hash(digest_input).to_bytes(),
            bare,
            "appending the caller-authority bump changed the digest its own address is \
             derived from. The carrier is unsatisfiable in that state -- see this test's \
             own doc comment."
        );
    }

    /// Every declared length, carried and uncarried, lands on its own route.
    #[test]
    fn the_carried_suffix_never_collides_with_another_exact_length() {
        let carried_widths = [
            dclutch_custody::CUSTODY_REQUEST_BYTES_V1,
            dclutch_custody::CUSTODY_REQUEST_BYTES_V1
                + REGISTRY_CONTINUATION_REQUEST_BYTES_V1,
            DELEGATED_CUSTODY_REQUEST_BYTES_V2,
        ];
        // The two routes that do NOT read a carried bump. A byte after one of
        // them must be left where it is, so the route refuses it exactly as it
        // did before this carrier existed rather than silently dropping it.
        let uncarried_widths = [
            PROJECTED_CUSTODY_REQUEST_BYTES_V1,
            dclutch_custody::RETIREMENT_REPLAY_HANDOFF_REQUEST_BYTES_V1,
        ];
        for width in carried_widths {
            // An exact width is handed on untouched -- no route loses its last
            // byte to a carrier that is not there.
            let bare = alloc::vec![0x11_u8; width];
            assert_eq!(
                split_caller_authority_bump_v1(&bare),
                (&bare[..], CustodyBumpRelayV1::ABSENT)
            );

            // One more is the ORIGINAL one-byte carrier, which still means the
            // caller authority and nothing else -- a wire emitted before the
            // relay widened keeps decoding to exactly what it always did.
            let mut carried = bare.clone();
            carried.push(0x22);
            let (stripped, relay) = split_caller_authority_bump_v1(&carried);
            assert_eq!(stripped.len(), width);
            assert_eq!(
                relay,
                CustodyBumpRelayV1 {
                    caller_authority: Some(0x22),
                    ..CustodyBumpRelayV1::ABSENT
                }
            );

            // Three more is the full relay, read in canonical order.
            let mut relayed = bare.clone();
            relayed.extend_from_slice(&[0x22, 0x33, 0x44]);
            let (stripped, relay) = split_caller_authority_bump_v1(&relayed);
            assert_eq!(stripped.len(), width);
            assert_eq!(
                relay,
                CustodyBumpRelayV1 {
                    caller_authority: Some(0x22),
                    replay: Some(0x33),
                    transfer_authority: Some(0x44),
                }
            );

            // Two more is neither width and must not be reinterpreted as one.
            let mut between = bare.clone();
            between.extend_from_slice(&[0x22, 0x33]);
            assert_eq!(
                split_caller_authority_bump_v1(&between),
                (&between[..], CustodyBumpRelayV1::ABSENT)
            );

            let short = alloc::vec![0x11_u8; width - 1];
            assert_eq!(
                split_caller_authority_bump_v1(&short),
                (&short[..], CustodyBumpRelayV1::ABSENT)
            );
        }
        for width in uncarried_widths {
            for length in [width, width + 1, width + CUSTODY_BUMP_RELAY_BYTES_V1] {
                let bytes = alloc::vec![0x11_u8; length];
                assert_eq!(
                    split_caller_authority_bump_v1(&bytes),
                    (&bytes[..], CustodyBumpRelayV1::ABSENT),
                    "a {length}-byte wire is not a carried shape and must reach dispatch whole"
                );
            }
        }
        // And no carried wire at either width can be mistaken for another
        // route's exact length, or for another carried wire's total.
        for width in carried_widths {
            for suffix in [1, CUSTODY_BUMP_RELAY_BYTES_V1] {
                for other in carried_widths.into_iter().chain(uncarried_widths) {
                    assert_ne!(width + suffix, other);
                    for other_suffix in [1, CUSTODY_BUMP_RELAY_BYTES_V1] {
                        assert!(
                            (other == width && other_suffix == suffix)
                                || width + suffix != other + other_suffix,
                            "two carried Custody wires share the total {}",
                            width + suffix
                        );
                    }
                }
            }
        }
        // A zero suffix byte is not a bump any derivation produces, so it reads
        // as absent rather than as a value certain to fail to reproduce -- in
        // every slot, so a partially mined caller relays what it has and the
        // rest searches.
        let mut zeroed = alloc::vec![0x11_u8; dclutch_custody::CUSTODY_REQUEST_BYTES_V1];
        zeroed.extend_from_slice(&[0, 0, 0]);
        assert_eq!(
            split_caller_authority_bump_v1(&zeroed).1,
            CustodyBumpRelayV1::ABSENT
        );
        let mut partial = alloc::vec![0x11_u8; dclutch_custody::CUSTODY_REQUEST_BYTES_V1];
        partial.extend_from_slice(&[0xfd, 0, 0xfb]);
        assert_eq!(
            split_caller_authority_bump_v1(&partial).1,
            CustodyBumpRelayV1 {
                caller_authority: Some(0xfd),
                replay: None,
                transfer_authority: Some(0xfb),
            }
        );
    }

    #[test]
    fn replay_rent_normalization_is_exact_and_overflow_safe() {
        const RENT: u64 = 1_000;
        assert_eq!(
            replay_rent_normalization(1, RENT, 7),
            Ok(ReplayRentNormalizationV1 {
                shortfall: 999,
                excess: 0,
            })
        );
        assert_eq!(
            replay_rent_normalization(RENT - 1, RENT, 7),
            Ok(ReplayRentNormalizationV1 {
                shortfall: 1,
                excess: 0,
            })
        );
        assert_eq!(
            replay_rent_normalization(RENT, RENT, 7),
            Ok(ReplayRentNormalizationV1 {
                shortfall: 0,
                excess: 0,
            })
        );
        assert_eq!(
            replay_rent_normalization(RENT + 1, RENT, 7),
            Ok(ReplayRentNormalizationV1 {
                shortfall: 0,
                excess: 1,
            })
        );
        assert_eq!(
            replay_rent_normalization(RENT + 1, RENT, u64::MAX),
            Err(CustodySbfError::Create.into())
        );
        assert_eq!(
            replay_rent_normalization(u64::MAX, RENT, 0),
            Ok(ReplayRentNormalizationV1 {
                shortfall: 0,
                excess: u64::MAX - RENT,
            })
        );
        assert_eq!(
            replay_rent_normalization(u64::MAX, RENT, RENT + 1),
            Err(CustodySbfError::Create.into())
        );
    }

    #[test]
    fn account_counts_are_operation_specific() {
        assert_eq!(INITIALIZE_REPLAY_ACCOUNT_COUNT_V1, 13);
        assert_eq!(OPEN_VAULT_ACCOUNT_COUNT_V1, 16);
        assert_eq!(TRANSFER_ACCOUNT_COUNT_V1, 14);
        assert_eq!(CLOSE_VAULT_ACCOUNT_COUNT_V1, 14);
        assert_eq!(CLOSE_REPLAY_ACCOUNT_COUNT_V1, 10);
    }

    fn premarket_series_initialize_request() -> CustodyRequestV1 {
        CustodyRequestV1 {
            operation: OperationV1::InitializeReplay,
            caller_role: CallerRoleV1::Trading,
            source_compartment: CompartmentV1::None,
            destination_compartment: CompartmentV1::None,
            release_set: [0x11; 32],
            market: [0x12; 32],
            realm: [0x13; 32],
            context: [0x14; 32],
            caller_program: [0x15; 32],
            semantic: dclutch_custody::ContextV1 {
                candidate: [0x16; 32],
                source_owner: [0; 32],
                destination_owner: [0; 32],
                order: [0x14; 32],
                parent_request_digest: [0x17; 32],
                order_nonce: 9,
                generation: 3,
                page_index: 0,
                execution_index: 9,
                transfer_index: 0,
            },
            source: [0; 32],
            destination: [0; 32],
            source_vault_context: [0; 32],
            destination_vault_context: [0; 32],
            mint: [0; 32],
            token_program: [0; 32],
            payer: [0x18; 32],
            rent_refund: [0x19; 32],
            expected_revision: 0,
            resulting_revision: 1,
            amount: 0,
            rent_lamports: 1,
        }
    }

    fn premarket_series_lock_request() -> CustodyRequestV1 {
        let mut request = premarket_series_initialize_request();
        request.operation = OperationV1::Transfer;
        request.source_compartment = CompartmentV1::External;
        request.destination_compartment = CompartmentV1::SeriesEscrow;
        request.semantic.source_owner = [0x20; 32];
        request.semantic.transfer_index = 2;
        request.source = [0x21; 32];
        request.destination = [0x22; 32];
        request.destination_vault_context = request.context;
        request.mint = [0x23; 32];
        request.token_program = [0x24; 32];
        request.payer = [0; 32];
        request.rent_refund = [0; 32];
        request.expected_revision = 2;
        request.resulting_revision = 3;
        request.amount = 41;
        request.rent_lamports = 0;
        request
    }

    #[test]
    fn only_exact_series_requests_with_a_system_empty_market_select_premarket() {
        let request = premarket_series_initialize_request();
        let market_key = Pubkey::new_from_array(request.market);
        let system_owner = system_program::ID;
        let mut lamports = 37;
        let mut empty_data = [];
        let market = AccountInfo::new(
            &market_key,
            false,
            false,
            &mut lamports,
            &mut empty_data,
            &system_owner,
            false,
        );
        assert!(is_premarket_series_vacancy(&market, request));
        assert_eq!(require_premarket_series_market(&market, request), Ok(()));

        let core_owner = Pubkey::new_from_array([0x31; 32]);
        let mut live_lamports = 38;
        let mut live_data = alloc::vec![0; STATE_BYTES];
        let live_market = AccountInfo::new(
            &market_key,
            false,
            false,
            &mut live_lamports,
            &mut live_data,
            &core_owner,
            false,
        );
        assert!(
            !is_premarket_series_vacancy(&live_market, request),
            "an allocated live Market must continue through authenticate_market"
        );

        let mut non_trading = request;
        non_trading.caller_role = CallerRoleV1::Core;
        assert!(!is_premarket_series_vacancy(&market, non_trading));
        let mut inexact = request;
        inexact.semantic.transfer_index = 1;
        assert!(!is_premarket_series_vacancy(&market, inexact));
    }

    #[test]
    fn selected_vacant_market_refuses_every_key_and_privilege_substitution() {
        let request = premarket_series_initialize_request();
        let key = Pubkey::new_from_array(request.market);
        let owner = system_program::ID;
        let mut lamports = 5;
        let mut data = [];
        let exact = AccountInfo::new(&key, false, false, &mut lamports, &mut data, &owner, false);
        let expected = Err(CustodySbfError::AccountFrame.into());

        let wrong_key = Pubkey::new_from_array([0x41; 32]);
        let mut wrong_key_lamports = 5;
        let mut wrong_key_data = [];
        let wrong_key_info = AccountInfo::new(
            &wrong_key,
            false,
            false,
            &mut wrong_key_lamports,
            &mut wrong_key_data,
            &owner,
            false,
        );
        assert_eq!(
            require_premarket_series_market(&wrong_key_info, request),
            expected
        );
        let mut signer = exact.clone();
        signer.is_signer = true;
        assert_eq!(require_premarket_series_market(&signer, request), expected);
        let mut writable = exact.clone();
        writable.is_writable = true;
        assert_eq!(
            require_premarket_series_market(&writable, request),
            expected
        );
        let mut executable = exact;
        executable.executable = true;
        assert_eq!(
            require_premarket_series_market(&executable, request),
            expected
        );
    }

    #[test]
    fn selected_vacancy_refusal_occurs_before_any_account_write() {
        let request = premarket_series_initialize_request();
        let owner = system_program::ID;
        let dummy_key = Pubkey::new_from_array([0x51; 32]);
        let mut dummy_lamports = 7;
        let mut dummy_data = [0x52];
        let dummy = AccountInfo::new(
            &dummy_key,
            false,
            false,
            &mut dummy_lamports,
            &mut dummy_data,
            &owner,
            false,
        );
        let wrong_market_key = Pubkey::new_from_array([0x53; 32]);
        let mut market_lamports = 11;
        let mut market_data = [];
        let market = AccountInfo::new(
            &wrong_market_key,
            false,
            false,
            &mut market_lamports,
            &mut market_data,
            &owner,
            false,
        );
        let accounts = [dummy, market];
        let before = (
            accounts[0].lamports(),
            accounts[0].try_borrow_data().expect("dummy data").to_vec(),
            accounts[1].lamports(),
            accounts[1].try_borrow_data().expect("market data").to_vec(),
        );
        assert_eq!(
            try_authenticate_premarket_market(&accounts, request),
            Err(CustodySbfError::AccountFrame.into())
        );
        assert_eq!(
            (
                accounts[0].lamports(),
                accounts[0].try_borrow_data().expect("dummy data").to_vec(),
                accounts[1].lamports(),
                accounts[1].try_borrow_data().expect("market data").to_vec(),
            ),
            before
        );
    }

    #[test]
    fn series_vacancy_keeps_replay_and_vault_pdas_as_commit_authorities() {
        let program = Pubkey::new_from_array([0x61; 32]);
        let initialize = premarket_series_initialize_request();
        let replay_key = Pubkey::find_program_address(
            &CustodyReplaySeedsV1::from_request(initialize).as_slices(),
            &program,
        )
        .0;
        let system_owner = system_program::ID;
        let mut replay_lamports = 0;
        let mut replay_data = [];
        let replay = AccountInfo::new(
            &replay_key,
            false,
            true,
            &mut replay_lamports,
            &mut replay_data,
            &system_owner,
            false,
        );
        assert_eq!(
            authenticate_replay_identity(&program, &replay, initialize, None),
            Ok(())
        );
        let wrong_replay_key = Pubkey::new_from_array([0x62; 32]);
        let mut wrong_replay_lamports = 0;
        let mut wrong_replay_data = [];
        let wrong_replay = AccountInfo::new(
            &wrong_replay_key,
            false,
            true,
            &mut wrong_replay_lamports,
            &mut wrong_replay_data,
            &system_owner,
            false,
        );
        assert_eq!(
            authenticate_replay_identity(&program, &wrong_replay, initialize, None),
            Err(CustodySbfError::Replay.into())
        );

        let lock = premarket_series_lock_request();
        let vault_key = Pubkey::find_program_address(
            &CustodyVaultSeedsV1::from_request(lock, false).as_slices(),
            &program,
        )
        .0;
        let mut vault_lamports = 0;
        let mut vault_data = [];
        let vault = AccountInfo::new(
            &vault_key,
            false,
            true,
            &mut vault_lamports,
            &mut vault_data,
            &system_owner,
            false,
        );
        assert_eq!(validate_vault_key(&program, &vault, lock, false), Ok(()));
        assert_eq!(
            validate_vault_key(&program, &wrong_replay, lock, false),
            Err(CustodySbfError::TokenState.into())
        );
    }

    fn continuation(roles: &[ExecutionRoleV1]) -> RegistryContinuationRequestV1 {
        RegistryContinuationRequestV1::new(
            ContentId::new([0x31; 32]).expect("release"),
            ContentId::new([0x32; 32]).expect("cache"),
            ContentId::new([0x33; 32]).expect("continuation"),
            1,
            ExecutionRoleV1::Core,
            roles,
        )
        .expect("syntactically valid continuation")
    }

    #[test]
    fn continuation_roles_are_operation_exact() {
        let open = continuation(&[ExecutionRoleV1::Core, ExecutionRoleV1::Custody]);
        for operation in [OperationV1::InitializeReplay, OperationV1::OpenVault] {
            assert_eq!(require_continuation_role_profile(operation, open), Ok(()));
        }

        let retirement = continuation(&[
            ExecutionRoleV1::Core,
            ExecutionRoleV1::Claims,
            ExecutionRoleV1::Resolution,
            ExecutionRoleV1::Custody,
        ]);
        for operation in [OperationV1::CloseVault, OperationV1::CloseReplay] {
            assert_eq!(
                require_continuation_role_profile(operation, retirement),
                Ok(())
            );
        }

        assert!(
            RegistryContinuationRequestV1::new(
                ContentId::new([0x31; 32]).expect("release"),
                ContentId::new([0x32; 32]).expect("cache"),
                ContentId::new([0x33; 32]).expect("continuation"),
                1,
                ExecutionRoleV1::Core,
                &[ExecutionRoleV1::Custody, ExecutionRoleV1::Core],
            )
            .is_err(),
            "the Registry contract refuses swapped role order before Custody"
        );
        for hostile in [
            continuation(&[
                ExecutionRoleV1::Core,
                ExecutionRoleV1::Claims,
                ExecutionRoleV1::Custody,
            ]),
            continuation(&[ExecutionRoleV1::Core]),
        ] {
            assert_eq!(
                require_continuation_role_profile(OperationV1::OpenVault, hostile),
                Err(CustodySbfError::Release.into())
            );
        }
        assert_eq!(
            require_continuation_role_profile(OperationV1::Transfer, open),
            Err(CustodySbfError::AccountFrame.into())
        );
    }

    #[test]
    fn finalized_realm_authority_refuses_every_substitution_axis() {
        let registry = Pubkey::new_from_array([0x31; 32]);
        let digest = [0x32; 32];
        let expected_realm = Pubkey::find_program_address(
            &[RAW_RECORD_PDA_SEED_V1, &REALM_SCHEMA_RELEASE_ID_V1, &digest],
            &registry,
        )
        .0;
        let expected_staging = Pubkey::find_program_address(
            &[
                STAGING_CURSOR_PDA_SEED_V1,
                &REALM_SCHEMA_RELEASE_ID_V1,
                &digest,
            ],
            &registry,
        )
        .0;
        let exact = RealmAuthorityObservation {
            registry,
            persisted_registry: registry,
            expected_realm,
            realm_key: expected_realm,
            realm_owner: registry,
            expected_staging,
            staging_key: expected_staging,
            staging_owner: system_program::ID,
            staging_data_len: 0,
            realm_digest: digest,
            expected_digest: digest,
        };
        assert_eq!(require_realm_authority(exact), Ok(()));
        for hostile in [
            RealmAuthorityObservation {
                realm_owner: Pubkey::new_unique(),
                ..exact
            },
            RealmAuthorityObservation {
                realm_key: Pubkey::new_unique(),
                ..exact
            },
            RealmAuthorityObservation {
                staging_owner: registry,
                ..exact
            },
            RealmAuthorityObservation {
                staging_key: Pubkey::new_unique(),
                ..exact
            },
            RealmAuthorityObservation {
                staging_data_len: 1,
                ..exact
            },
            RealmAuthorityObservation {
                registry: Pubkey::new_unique(),
                ..exact
            },
            RealmAuthorityObservation {
                realm_digest: [0x41; 32],
                ..exact
            },
            RealmAuthorityObservation {
                expected_digest: [0x42; 32],
                ..exact
            },
        ] {
            assert_eq!(
                require_realm_authority(hostile),
                Err(CustodySbfError::Realm.into())
            );
        }
    }

    #[test]
    fn role_mapping_never_lends_custody_to_itself() {
        assert_eq!(registry_role(CallerRoleV1::Core), ExecutionRoleV1::Core);
        assert_eq!(registry_role(CallerRoleV1::Claims), ExecutionRoleV1::Claims);
        assert_eq!(
            registry_role(CallerRoleV1::Trading),
            ExecutionRoleV1::Trading
        );
        assert_eq!(
            registry_role(CallerRoleV1::Resolution),
            ExecutionRoleV1::Resolution
        );
    }

    #[test]
    fn both_realm_profiles_are_pinned_by_release_preimage_digest() {
        for release in PRODUCTION_ADAPTER_RELEASES {
            let encoded = release.to_bytes();
            assert_ne!(hash(&encoded).to_bytes(), [0; 32]);
            assert_eq!(
                dclutch_custody::token_svm::CollateralAdapterReleaseV1::decode(&encoded),
                Ok(release)
            );
        }
    }

    /// The relayed replay bump reproduces this program's own coordinate, and a
    /// wrong one names an account this transaction was not handed.
    ///
    /// `authenticate_replay_identity` compares what it derives against the
    /// account at the replay coordinate, so a hint that is not the canonical
    /// bump can only produce an address that comparison refuses. That is the
    /// entire safety argument for taking the byte off the wire, and it does not
    /// weaken because the byte crossed a CPI boundary to get here.
    #[test]
    fn a_wrong_relayed_replay_bump_reproduces_another_address_and_refuses() {
        let program = Pubkey::new_from_array([0x61; 32]);
        let seeds =
            CustodyReplaySeedsV1::new([0x62; 32], [0x63; 32], CallerRoleV1::Trading, [0x64; 32]);
        let (canonical_address, canonical) =
            Pubkey::find_program_address(&seeds.as_slices(), &program);
        let at = |bump: u8| {
            let bump_seed = [bump];
            let [domain, market, release, role, context] = seeds.as_slices();
            Pubkey::create_program_address(
                &[domain, market, release, role, context, &bump_seed],
                &program,
            )
        };
        assert_eq!(at(canonical), Ok(canonical_address));
        let mut refused = 0_u32;
        for bump in 0..=u8::MAX {
            if bump == canonical {
                continue;
            }
            if let Ok(address) = at(bump) {
                assert_ne!(address, canonical_address, "bump {bump}");
            }
            refused = refused.saturating_add(1);
        }
        assert_eq!(refused, 255);
    }

    /// The relayed transfer-authority bump is the byte this program SIGNS with,
    /// and that is exactly why a wrong one is harmless.
    ///
    /// `validate_custody_authority` derives the address before it returns the
    /// bump and refuses unless it is the authority account the frame supplied.
    /// A non-canonical byte therefore never reaches `invoke_signed`: the route
    /// has already refused. Nothing signs with a caller-named seed.
    #[test]
    fn a_wrong_relayed_transfer_authority_bump_never_reaches_a_signature() {
        let program = Pubkey::new_from_array([0x71; 32]);
        let seeds = CustodyAuthoritySeedsV1::new([0x72; 32], [0x73; 32]);
        let (canonical_address, canonical) =
            Pubkey::find_program_address(&seeds.as_slices(), &program);
        let at = |bump: u8| {
            let bump_seed = [bump];
            let [domain, market, release] = seeds.as_slices();
            Pubkey::create_program_address(&[domain, market, release, &bump_seed], &program)
        };
        assert_eq!(at(canonical), Ok(canonical_address));
        let mut refused = 0_u32;
        for bump in 0..=u8::MAX {
            if bump == canonical {
                continue;
            }
            if let Ok(address) = at(bump) {
                assert_ne!(address, canonical_address, "bump {bump}");
            }
            refused = refused.saturating_add(1);
        }
        assert_eq!(refused, 255);
    }

    #[test]
    fn authenticated_authority_bump_reproduces_exact_key_and_wrong_bump_does_not() {
        let program = Pubkey::new_from_array([0x51; 32]);
        let seeds = CustodyAuthoritySeedsV1::new([0x52; 32], [0x53; 32]);
        let (canonical, bump) = Pubkey::find_program_address(&seeds.as_slices(), &program);
        let witness = AuthenticatedCustodyAuthorityBumpV1(bump);
        let [domain, market, release] = seeds.as_slices();
        assert_eq!(
            Pubkey::create_program_address(&[domain, market, release, &[witness.0]], &program,),
            Ok(canonical),
        );
        let wrong = witness.0.wrapping_sub(1);
        assert_ne!(
            Pubkey::create_program_address(&[domain, market, release, &[wrong]], &program).ok(),
            Some(canonical),
        );
    }
}
