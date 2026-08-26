//! Joined Retiring-to-Retired physical waist.
//!
//! The instruction carries the fixed Core request, one retirement bundle, the
//! Claims aggregate-close request, and the ordered normal-Custody CloseVault
//! and CloseReplay requests. The adapter authenticates the already-persisted
//! Resolution closure, invokes Claims then both Custody closes, verifies every
//! immediate typed receipt and physical poststate, runs the generated
//! [`dclutch_market_core_codec::retire`] transition on a local candidate, and
//! closes the Core Market to RentCredit last.

extern crate alloc;

use alloc::vec::Vec;

use dclutch_claims_svm::market_closure_v1::{
    CLAIMS_MARKET_CLOSURE_POST_RESOURCE_DIGEST_DOMAIN_V1,
    CLAIMS_MARKET_CLOSURE_PRE_RESOURCE_DIGEST_DOMAIN_V1, CLAIMS_MARKET_CLOSURE_RECEIPT_BYTES_V1,
    CLAIMS_MARKET_CLOSURE_REQUEST_BYTES_V1, ClaimsMarketClosureReceiptV1,
    ClaimsMarketClosureRequestV1,
};
use dclutch_custody_contract::{
    CUSTODY_POSTSTATE_DOMAIN_V1, CUSTODY_RECEIPT_BYTES_V1, CUSTODY_REPLAY_BYTES_V1,
    CUSTODY_REQUEST_BYTES_V1, CallerRoleV1, CompartmentV1, CustodyReceiptV1, CustodyReplayV1,
    CustodyRequestV1, OperationV1,
};
use dclutch_market_core_codec::{
    ChildEffectObservation, ClaimsEffectObservation, CoreState, MarketCoreStateSeedsV2, Phase,
    REQUEST_BYTES, RETIRED_CANDIDATE_DIGEST_DOMAIN_V1, RETIREMENT_BUNDLE_BYTES_V1,
    RETIREMENT_CUSTODY_RECEIPT_COUNT_V1, RETIREMENT_POST_RESOURCE_DIGEST_DOMAIN_V1,
    RETIREMENT_RECEIPT_BYTES_V1, RETIREMENT_ROLE_COUNT_V1, Request, RetirementBundleV1,
    RetirementReceiptInputV1, RetirementReceiptV1, Role, STATE_BYTES, retire,
};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_rent_contract::lifecycle_v2::{
    CloseLifecycleRentCreditV2, LIFECYCLE_RENT_CLOSE_RECEIPT_BYTES_V2, LifecycleAccountIdV2,
    LifecycleRentCloseReceiptV2, LifecycleRentCoreCloseAuthoritySeedsV2, LifecycleRentCreditV2,
};
use dclutch_resolution_codec::{
    SOURCE_CLOSURE_RECEIPT_BYTES_V2, SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V2, SourceClosureReceiptV2,
};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed, set_return_data},
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{system_program, sysvar};

use crate::{
    CoreSbfError, infrastructure,
    release::{RoleDeploymentAccounts, authenticate_roles},
};

/// Exact joined retirement instruction width.
pub const RETIREMENT_INSTRUCTION_BYTES_V1: usize = REQUEST_BYTES
    + RETIREMENT_BUNDLE_BYTES_V1
    + CLAIMS_MARKET_CLOSURE_REQUEST_BYTES_V1
    + CUSTODY_REQUEST_BYTES_V1
    + CUSTODY_REQUEST_BYTES_V1;

/// Writable Core Market.
pub const MARKET_ACCOUNT_V1: usize = 0;
/// Writable immutable RentCredit.
pub const RENT_CREDIT_ACCOUNT_V1: usize = 1;
/// Registry activation cache.
pub const ACTIVATION_CACHE_ACCOUNT_V1: usize = 2;
/// Immutable Registry program.
pub const REGISTRY_PROGRAM_ACCOUNT_V1: usize = 3;
/// Current Core program.
pub const CORE_PROGRAM_ACCOUNT_V1: usize = 4;
/// Current Core ProgramData.
pub const CORE_PROGRAMDATA_ACCOUNT_V1: usize = 5;
/// Current Claims program.
pub const CLAIMS_PROGRAM_ACCOUNT_V1: usize = 6;
/// Current Claims ProgramData.
pub const CLAIMS_PROGRAMDATA_ACCOUNT_V1: usize = 7;
/// Current Resolution program.
pub const RESOLUTION_PROGRAM_ACCOUNT_V1: usize = 8;
/// Current Resolution ProgramData.
pub const RESOLUTION_PROGRAMDATA_ACCOUNT_V1: usize = 9;
/// Current Custody program.
pub const CUSTODY_PROGRAM_ACCOUNT_V1: usize = 10;
/// Current Custody ProgramData.
pub const CUSTODY_PROGRAMDATA_ACCOUNT_V1: usize = 11;
/// Immutable infrastructure-selected Rent program.
pub const RENT_PROGRAM_ACCOUNT_V1: usize = 12;
/// Persisted Resolution closure receipt.
pub const SOURCE_RECEIPT_ACCOUNT_V1: usize = 13;
/// Writable Claims aggregate.
pub const CLAIMS_AGGREGATE_ACCOUNT_V1: usize = 14;
/// Writable normal Custody replay.
pub const CUSTODY_REPLAY_ACCOUNT_V1: usize = 15;
/// Writable canonical HoardPrincipal vault.
pub const HOARD_VAULT_ACCOUNT_V1: usize = 16;
/// Custody token authority PDA.
pub const CUSTODY_AUTHORITY_ACCOUNT_V1: usize = 17;
/// Realm-selected collateral Mint.
pub const COLLATERAL_MINT_ACCOUNT_V1: usize = 18;
/// Realm-selected collateral token program.
pub const COLLATERAL_TOKEN_PROGRAM_ACCOUNT_V1: usize = 19;
/// Finalized Realm raw record.
pub const REALM_RAW_ACCOUNT_V1: usize = 20;
/// Vacant finalized Realm staging cursor.
pub const REALM_STAGING_ACCOUNT_V1: usize = 21;
/// Core caller PDA for Claims closure.
pub const CLAIMS_CALLER_AUTHORITY_ACCOUNT_V1: usize = 22;
/// Core caller PDA for Custody CloseVault.
pub const CUSTODY_CLOSE_VAULT_AUTHORITY_ACCOUNT_V1: usize = 23;
/// Core caller PDA for Custody CloseReplay.
pub const CUSTODY_CLOSE_REPLAY_AUTHORITY_ACCOUNT_V1: usize = 24;
/// Immutable Core infrastructure profile.
pub const INFRASTRUCTURE_PROFILE_ACCOUNT_V1: usize = 25;
/// Finalized Registry ArtifactRelease raw record.
pub const REGISTRY_ARTIFACT_RAW_ACCOUNT_V1: usize = 26;
/// Vacant Registry ArtifactRelease staging cursor.
pub const REGISTRY_ARTIFACT_STAGING_ACCOUNT_V1: usize = 27;
/// Current Registry ProgramData.
pub const REGISTRY_PROGRAMDATA_ACCOUNT_V1: usize = 28;
/// Finalized Rent ArtifactRelease raw record.
pub const RENT_ARTIFACT_RAW_ACCOUNT_V1: usize = 29;
/// Vacant Rent ArtifactRelease staging cursor.
pub const RENT_ARTIFACT_STAGING_ACCOUNT_V1: usize = 30;
/// Current Rent ProgramData.
pub const RENT_PROGRAMDATA_ACCOUNT_V1: usize = 31;
/// Rent sysvar.
pub const RENT_SYSVAR_ACCOUNT_V1: usize = 32;
/// Writable immutable lifecycle refund wallet.
pub const RENT_REFUND_WALLET_ACCOUNT_V1: usize = 33;
/// Core-derived caller authority for RentCredit closure.
pub const RENT_CLOSE_AUTHORITY_ACCOUNT_V1: usize = 34;
/// Exact joined retirement account count.
pub const RETIREMENT_ACCOUNT_COUNT_V1: usize = 35;

#[derive(Clone, Copy)]
struct RetirementAccounts<'accounts, 'info> {
    market: &'accounts AccountInfo<'info>,
    rent_credit: &'accounts AccountInfo<'info>,
    cache: &'accounts AccountInfo<'info>,
    registry: &'accounts AccountInfo<'info>,
    core_program: &'accounts AccountInfo<'info>,
    core_programdata: &'accounts AccountInfo<'info>,
    claims_program: &'accounts AccountInfo<'info>,
    claims_programdata: &'accounts AccountInfo<'info>,
    resolution_program: &'accounts AccountInfo<'info>,
    resolution_programdata: &'accounts AccountInfo<'info>,
    custody_program: &'accounts AccountInfo<'info>,
    custody_programdata: &'accounts AccountInfo<'info>,
    rent_program: &'accounts AccountInfo<'info>,
    source_receipt: &'accounts AccountInfo<'info>,
    claims_aggregate: &'accounts AccountInfo<'info>,
    custody_replay: &'accounts AccountInfo<'info>,
    hoard_vault: &'accounts AccountInfo<'info>,
    custody_authority: &'accounts AccountInfo<'info>,
    collateral_mint: &'accounts AccountInfo<'info>,
    token_program: &'accounts AccountInfo<'info>,
    realm_raw: &'accounts AccountInfo<'info>,
    realm_staging: &'accounts AccountInfo<'info>,
    claims_authority: &'accounts AccountInfo<'info>,
    close_vault_authority: &'accounts AccountInfo<'info>,
    close_replay_authority: &'accounts AccountInfo<'info>,
    infrastructure_profile: &'accounts AccountInfo<'info>,
    registry_artifact_raw: &'accounts AccountInfo<'info>,
    registry_artifact_staging: &'accounts AccountInfo<'info>,
    registry_programdata: &'accounts AccountInfo<'info>,
    rent_artifact_raw: &'accounts AccountInfo<'info>,
    rent_artifact_staging: &'accounts AccountInfo<'info>,
    rent_programdata: &'accounts AccountInfo<'info>,
    rent: &'accounts AccountInfo<'info>,
    refund_wallet: &'accounts AccountInfo<'info>,
    rent_close_authority: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> RetirementAccounts<'accounts, 'info> {
    fn parse(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, CoreSbfError> {
        let [
            market,
            rent_credit,
            cache,
            registry,
            core_program,
            core_programdata,
            claims_program,
            claims_programdata,
            resolution_program,
            resolution_programdata,
            custody_program,
            custody_programdata,
            rent_program,
            source_receipt,
            claims_aggregate,
            custody_replay,
            hoard_vault,
            custody_authority,
            collateral_mint,
            token_program,
            realm_raw,
            realm_staging,
            claims_authority,
            close_vault_authority,
            close_replay_authority,
            infrastructure_profile,
            registry_artifact_raw,
            registry_artifact_staging,
            registry_programdata,
            rent_artifact_raw,
            rent_artifact_staging,
            rent_programdata,
            rent,
            refund_wallet,
            rent_close_authority,
        ] = accounts
        else {
            return Err(CoreSbfError::AccountFrame);
        };
        Ok(Self {
            market,
            rent_credit,
            cache,
            registry,
            core_program,
            core_programdata,
            claims_program,
            claims_programdata,
            resolution_program,
            resolution_programdata,
            custody_program,
            custody_programdata,
            rent_program,
            source_receipt,
            claims_aggregate,
            custody_replay,
            hoard_vault,
            custody_authority,
            collateral_mint,
            token_program,
            realm_raw,
            realm_staging,
            claims_authority,
            close_vault_authority,
            close_replay_authority,
            infrastructure_profile,
            registry_artifact_raw,
            registry_artifact_staging,
            registry_programdata,
            rent_artifact_raw,
            rent_artifact_staging,
            rent_programdata,
            rent,
            refund_wallet,
            rent_close_authority,
        })
    }
}

struct RetirementEvidence {
    source_digest: [u8; 32],
    claims_digest: [u8; 32],
    close_vault_digest: [u8; 32],
    close_replay_digest: [u8; 32],
    source_revision: u64,
    claims_revision: u64,
    custody_revision: u64,
    claims_refund: u64,
    custody_refund: u64,
}

#[derive(Clone, Copy)]
struct ClaimsCloseEvidence {
    digest: [u8; 32],
    revision: u64,
    refund: u64,
}

#[derive(Clone, Copy)]
struct CustodyRequestJoin {
    context: [u8; 32],
    realm: [u8; 32],
    candidate: [u8; 32],
    order: [u8; 32],
    order_nonce: u64,
    page_index: u32,
    execution_index: u32,
}

#[derive(Clone, Copy)]
struct CustodyCloseEvidence {
    digest: [u8; 32],
    refund: u64,
    join: CustodyRequestJoin,
}

#[derive(Clone, Copy)]
struct CustodyTerminalEvidence {
    digest: [u8; 32],
    revision: u64,
    refund: u64,
}

#[derive(Clone, Copy)]
struct RetiredTransitionPlan {
    core_refund: u64,
    candidate_digest: [u8; 32],
}

/// Execute the one canonical joined Market retirement.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: Request,
    request_bytes: &[u8],
    bundle_bytes: &[u8],
    claims_request_bytes: &[u8],
    close_vault_request_bytes: &[u8],
    close_replay_request_bytes: &[u8],
) -> ProgramResult {
    let frame = RetirementAccounts::parse(accounts)?;
    authenticate_privileges(program_id, frame)?;
    let rent = Rent::from_account_info(frame.rent).map_err(|_| CoreSbfError::Infrastructure)?;
    infrastructure::authenticate_profile(
        program_id,
        frame.infrastructure_profile,
        frame.registry_artifact_raw,
        frame.registry_artifact_staging,
        frame.registry,
        frame.registry_programdata,
        frame.rent_artifact_raw,
        frame.rent_artifact_staging,
        frame.rent_program,
        frame.rent_programdata,
        &rent,
    )?;
    process_authenticated(
        program_id,
        frame,
        request,
        request_bytes,
        bundle_bytes,
        claims_request_bytes,
        close_vault_request_bytes,
        close_replay_request_bytes,
        &rent,
    )
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn process_authenticated(
    program_id: &Pubkey,
    frame: RetirementAccounts<'_, '_>,
    request: Request,
    request_bytes: &[u8],
    bundle_bytes: &[u8],
    claims_request_bytes: &[u8],
    close_vault_request_bytes: &[u8],
    close_replay_request_bytes: &[u8],
    rent: &Rent,
) -> ProgramResult {
    let bundle = RetirementBundleV1::decode(bundle_bytes).map_err(|_| CoreSbfError::Instruction)?;
    let bundle_input = bundle.input_ref();
    let state = authenticate_market(program_id, frame, request, bundle_input)?;
    let admissions = authenticate_roles(
        frame.cache,
        frame.registry,
        state.identity.registry_program,
        bundle_input.release_set,
        &[
            RoleDeploymentAccounts::new(Role::Core, frame.core_program, frame.core_programdata),
            RoleDeploymentAccounts::new(
                Role::Claims,
                frame.claims_program,
                frame.claims_programdata,
            ),
            RoleDeploymentAccounts::new(
                Role::Resolution,
                frame.resolution_program,
                frame.resolution_programdata,
            ),
            RoleDeploymentAccounts::new(
                Role::Custody,
                frame.custody_program,
                frame.custody_programdata,
            ),
        ],
    )
    .map_err(|_| CoreSbfError::Instruction)?;
    authenticate_rent_credit(frame, state, bundle_input.rent_credit)?;
    let parent_digest = hash(request_bytes).to_bytes();
    let source_digest = authenticate_source_receipt(frame, state, bundle_input, rent)?;
    let rent_before = frame.rent_credit.lamports();
    let claims = execute_claims(
        program_id,
        frame,
        bundle_input,
        claims_request_bytes,
        parent_digest,
    )?;
    let close_vault = execute_close_vault(
        program_id,
        frame,
        state,
        bundle_input,
        close_vault_request_bytes,
        parent_digest,
    )?;
    let close_replay = execute_close_replay(
        program_id,
        frame,
        state,
        bundle_input,
        close_replay_request_bytes,
        parent_digest,
        close_vault.join,
    )?;
    let evidence = RetirementEvidence {
        source_digest,
        claims_digest: claims.digest,
        close_vault_digest: close_vault.digest,
        close_replay_digest: close_replay.digest,
        source_revision: bundle_input.source_closure_revision,
        claims_revision: claims.revision,
        custody_revision: close_replay.revision,
        claims_refund: claims.refund,
        custody_refund: close_vault
            .refund
            .checked_add(close_replay.refund)
            .ok_or(CoreSbfError::Arithmetic)?,
    };
    let transition = plan_retired_transition(
        request,
        state,
        admissions,
        bundle_input.expected_core_lamports,
    )?;
    commit_retired(
        program_id,
        frame,
        &bundle,
        bundle_bytes,
        evidence,
        rent_before,
        transition,
    )
}

#[inline(never)]
fn authenticate_market(
    program_id: &Pubkey,
    frame: RetirementAccounts<'_, '_>,
    request: Request,
    bundle: &dclutch_market_core_codec::RetirementBundleInputV1,
) -> Result<CoreState, CoreSbfError> {
    if frame.market.owner != program_id
        || frame.market.data_len() != STATE_BYTES
        || frame.market.key.to_bytes() != bundle.market
        || frame.market.lamports() != bundle.expected_core_lamports
    {
        return Err(CoreSbfError::Market);
    }
    let data = frame
        .market
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Market)?;
    let bytes: [u8; STATE_BYTES] = data.as_ref().try_into().map_err(|_| CoreSbfError::Market)?;
    let state = CoreState::decode(&bytes).map_err(|_| CoreSbfError::Market)?;
    let expected = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(state.identity).as_slices(),
        program_id,
    )
    .0;
    if expected != *frame.market.key
        || frame.core_program.key != program_id
        || state.phase != Phase::Retiring
        || state.identity.market_id.to_bytes() != bundle.market
        || state.identity.selected_release_set.to_bytes() != bundle.release_set
        || state.identity.generation != bundle.generation
        || state.identity.registry_program.to_bytes() != frame.registry.key.to_bytes()
        || state.outstanding_capabilities != 0
        || request.market.to_bytes() != bundle.market
        || request.generation != bundle.generation
        || hash(&bytes).to_bytes() != bundle.core_prestate_digest
    {
        return Err(CoreSbfError::Market);
    }
    Ok(state)
}

#[inline(never)]
fn authenticate_rent_credit(
    frame: RetirementAccounts<'_, '_>,
    state: CoreState,
    expected_account: [u8; 32],
) -> ProgramResult {
    if frame.rent_credit.key.to_bytes() != expected_account
        || frame.rent_credit.owner != frame.rent_program.key
    {
        return Err(CoreSbfError::RentCredit.into());
    }
    let data = frame
        .rent_credit
        .try_borrow_data()
        .map_err(|_| CoreSbfError::RentCredit)?;
    let credit = LifecycleRentCreditV2::decode(&data).map_err(|_| CoreSbfError::RentCredit)?;
    let seeds = credit.pda_seeds();
    let authority = credit.refund_wallet().to_bytes();
    let bump = [seeds.bump()];
    let generation = seeds.generation();
    let expected = Pubkey::create_program_address(
        &[
            seeds.domain(),
            seeds.market().to_bytes().as_slice(),
            generation.as_slice(),
            &bump,
        ],
        frame.rent_program.key,
    )
    .map_err(|_| CoreSbfError::RentCredit)?;
    if expected != *frame.rent_credit.key
        || frame.rent_credit.key.to_bytes() != state.rent_beneficiary.to_bytes()
        || frame.refund_wallet.key.to_bytes() != authority
        || frame.refund_wallet.owner != &system_program::ID
        || !frame.refund_wallet.data_is_empty()
        || credit.market().to_bytes() != state.identity.market_id.to_bytes()
        || credit.release_set().to_bytes() != state.identity.selected_release_set.to_bytes()
        || credit.generation() != state.identity.generation
    {
        return Err(CoreSbfError::RentCredit.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn require_custody_request(
    program_id: &Pubkey,
    frame: RetirementAccounts<'_, '_>,
    state: CoreState,
    bundle: &dclutch_market_core_codec::RetirementBundleInputV1,
    request: CustodyRequestV1,
    request_bytes: &[u8],
    operation: OperationV1,
    source_compartment: CompartmentV1,
    expected_revision: u64,
    resulting_revision: u64,
    expected_digest: [u8; 32],
    parent_digest: [u8; 32],
    transfer_index: u16,
) -> ProgramResult {
    let close_vault = operation == OperationV1::CloseVault;
    if request.operation != operation
        || request.caller_role != CallerRoleV1::Core
        || request.caller_program != program_id.to_bytes()
        || request.release_set != bundle.release_set
        || request.market != bundle.market
        || request.realm != state.identity.realm_id.to_bytes()
        || request.source_compartment != source_compartment
        || request.destination_compartment != CompartmentV1::None
        || request.semantic.parent_request_digest != parent_digest
        || request.semantic.generation != bundle.generation
        || request.semantic.transfer_index != transfer_index
        || request.expected_revision != expected_revision
        || request.resulting_revision != resulting_revision
        || request.amount != 0
        || request.rent_refund != bundle.rent_credit
        || hash(request_bytes).to_bytes() != expected_digest
        || (close_vault
            && (request.source != bundle.hoard_vault
                || request.source_vault_context != request.context
                || request.mint != frame.collateral_mint.key.to_bytes()
                || request.token_program != frame.token_program.key.to_bytes()
                || request.rent_lamports != frame.hoard_vault.lamports()))
        || (!close_vault
            && (request.source != [0; 32]
                || request.source_vault_context != [0; 32]
                || request.mint != [0; 32]
                || request.token_program != [0; 32]
                || request.rent_lamports != frame.custody_replay.lamports()))
    {
        return Err(CoreSbfError::Instruction.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_source_receipt(
    frame: RetirementAccounts<'_, '_>,
    state: CoreState,
    bundle: &dclutch_market_core_codec::RetirementBundleInputV1,
    rent: &Rent,
) -> Result<[u8; 32], CoreSbfError> {
    if frame.source_receipt.owner != frame.resolution_program.key
        || frame.source_receipt.key.to_bytes() != bundle.source_receipt_account
        || frame.source_receipt.data_len() != SOURCE_CLOSURE_RECEIPT_BYTES_V2
        || !rent.is_exempt(
            frame.source_receipt.lamports(),
            SOURCE_CLOSURE_RECEIPT_BYTES_V2,
        )
    {
        return Err(CoreSbfError::ChildAck);
    }
    let bytes = frame
        .source_receipt
        .try_borrow_data()
        .map_err(|_| CoreSbfError::ChildAck)?;
    let digest = hash(&bytes).to_bytes();
    let receipt = SourceClosureReceiptV2::decode(&bytes).map_err(|_| CoreSbfError::ChildAck)?;
    let terminal = state
        .terminal_receipt
        .ok_or(CoreSbfError::Transition)?
        .to_bytes();
    let receipt_sequence = receipt
        .terminal_sequence
        .checked_add(1)
        .ok_or(CoreSbfError::Arithmetic)?;
    if digest != bundle.source_receipt_digest
        || receipt.market != bundle.market
        || receipt.receipt_account != bundle.source_receipt_account
        || receipt.capability_manifest != state.identity.capability_manifest.to_bytes()
        || receipt.terminal_certificate != terminal
        || receipt.beneficiary != state.rent_beneficiary.to_bytes()
        || receipt.generation != bundle.generation
        || receipt.selector != state.terminal_winner
        || receipt_sequence != bundle.source_closure_revision
    {
        return Err(CoreSbfError::ChildAck);
    }
    let sequence = receipt_sequence.to_le_bytes();
    let expected = Pubkey::find_program_address(
        &[
            SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V2,
            receipt.source_state.as_slice(),
            &sequence,
        ],
        frame.resolution_program.key,
    )
    .0;
    if expected != *frame.source_receipt.key {
        return Err(CoreSbfError::ChildAck);
    }
    Ok(digest)
}

#[inline(never)]
fn execute_claims(
    program_id: &Pubkey,
    frame: RetirementAccounts<'_, '_>,
    bundle: &dclutch_market_core_codec::RetirementBundleInputV1,
    request_bytes: &[u8],
    parent_digest: [u8; 32],
) -> Result<ClaimsCloseEvidence, CoreSbfError> {
    let request = ClaimsMarketClosureRequestV1::decode(request_bytes)
        .map_err(|_| CoreSbfError::Instruction)?;
    let request_input = request.input();
    if request_input.release_set != bundle.release_set
        || request_input.market != bundle.market
        || request_input.aggregate != bundle.claims_aggregate
        || request_input.rent_credit != bundle.rent_credit
        || request_input.parent_request_digest != parent_digest
        || request_input.core_program != program_id.to_bytes()
        || request_input.generation != bundle.generation
        || request_input.expected_revision != bundle.claims_pre_revision
        || request_input.resulting_revision != bundle.claims_post_revision
        || hash(request_bytes).to_bytes() != bundle.claims_request_digest
        || frame.claims_aggregate.key.to_bytes() != bundle.claims_aggregate
    {
        return Err(CoreSbfError::Instruction);
    }
    let pre_bytes = frame
        .claims_aggregate
        .try_borrow_data()
        .map_err(|_| CoreSbfError::ChildAck)?;
    let pre_digest = hashv(&[
        CLAIMS_MARKET_CLOSURE_PRE_RESOURCE_DIGEST_DOMAIN_V1.as_slice(),
        frame.claims_aggregate.key.as_ref(),
        pre_bytes.as_ref(),
    ])
    .to_bytes();
    drop(pre_bytes);
    let refund = frame.claims_aggregate.lamports();
    let credit_after = frame
        .rent_credit
        .lamports()
        .checked_add(refund)
        .ok_or(CoreSbfError::Arithmetic)?;
    invoke_child(
        program_id,
        frame.claims_program,
        frame.claims_authority,
        request_input.release_set,
        request_input.market,
        request_input.parent_request_digest,
        request_bytes,
        &[
            (frame.claims_authority, false, true),
            (frame.claims_aggregate, true, false),
            (frame.rent_credit, true, false),
            (frame.cache, false, false),
            (frame.registry, false, false),
            (frame.claims_program, false, false),
            (frame.claims_programdata, false, false),
            (frame.core_program, false, false),
            (frame.core_programdata, false, false),
            (frame.market, false, false),
            (frame.rent_program, false, false),
        ],
    )
    .map_err(|_| CoreSbfError::ChildCpi)?;
    let (producer, receipt_bytes) = get_return_data().ok_or(CoreSbfError::ChildAck)?;
    if producer != *frame.claims_program.key
        || receipt_bytes.len() != CLAIMS_MARKET_CLOSURE_RECEIPT_BYTES_V1
        || frame.claims_aggregate.owner != &system_program::ID
        || !frame.claims_aggregate.data_is_empty()
        || frame.claims_aggregate.lamports() != 0
        || frame.rent_credit.lamports() != credit_after
    {
        return Err(CoreSbfError::ChildAck);
    }
    let post_digest = hashv(&[
        CLAIMS_MARKET_CLOSURE_POST_RESOURCE_DIGEST_DOMAIN_V1.as_slice(),
        frame.claims_aggregate.key.as_ref(),
        frame.rent_credit.key.as_ref(),
        request_input.resulting_revision.to_le_bytes().as_slice(),
        refund.to_le_bytes().as_slice(),
        credit_after.to_le_bytes().as_slice(),
    ])
    .to_bytes();
    let receipt =
        ClaimsMarketClosureReceiptV1::decode(&receipt_bytes).map_err(|_| CoreSbfError::ChildAck)?;
    let request_digest = hash(request_bytes).to_bytes();
    receipt
        .verify_for(request, request_digest, pre_digest, post_digest)
        .map_err(|_| CoreSbfError::ChildAck)?;
    if receipt.input().producer != frame.claims_program.key.to_bytes()
        || receipt.input().liability_units != 0
        || receipt.input().refund_lamports != refund
    {
        return Err(CoreSbfError::ChildAck);
    }
    Ok(ClaimsCloseEvidence {
        digest: hash(&receipt_bytes).to_bytes(),
        revision: receipt.input().post_revision,
        refund: receipt.input().refund_lamports,
    })
}

#[inline(never)]
fn execute_close_vault(
    program_id: &Pubkey,
    frame: RetirementAccounts<'_, '_>,
    state: CoreState,
    bundle: &dclutch_market_core_codec::RetirementBundleInputV1,
    request_bytes: &[u8],
    parent_digest: [u8; 32],
) -> Result<CustodyCloseEvidence, CoreSbfError> {
    let request = CustodyRequestV1::decode(request_bytes).map_err(|_| CoreSbfError::Instruction)?;
    require_custody_request(
        program_id,
        frame,
        state,
        bundle,
        request,
        request_bytes,
        OperationV1::CloseVault,
        CompartmentV1::HoardPrincipal,
        bundle.custody_pre_revision,
        bundle.custody_middle_revision,
        bundle.custody_close_vault_request_digest,
        parent_digest,
        0,
    )
    .map_err(|_| CoreSbfError::Instruction)?;
    let credit_after = frame
        .rent_credit
        .lamports()
        .checked_add(request.rent_lamports)
        .ok_or(CoreSbfError::Arithmetic)?;
    invoke_child(
        program_id,
        frame.custody_program,
        frame.close_vault_authority,
        request.release_set,
        request.market,
        request.context,
        request_bytes,
        &[
            (frame.close_vault_authority, false, true),
            (frame.market, false, false),
            (frame.cache, false, false),
            (frame.registry, false, false),
            (frame.core_program, false, false),
            (frame.core_programdata, false, false),
            (frame.realm_raw, false, false),
            (frame.realm_staging, false, false),
            (frame.custody_replay, true, false),
            (frame.collateral_mint, false, false),
            (frame.hoard_vault, true, false),
            (frame.custody_authority, false, false),
            (frame.token_program, false, false),
            (frame.rent_credit, true, false),
        ],
    )
    .map_err(|_| CoreSbfError::ChildCpi)?;
    let (producer, receipt_bytes) = get_return_data().ok_or(CoreSbfError::ChildAck)?;
    if producer != *frame.custody_program.key
        || receipt_bytes.len() != CUSTODY_RECEIPT_BYTES_V1
        || frame.hoard_vault.lamports() != 0
        || frame.rent_credit.lamports() != credit_after
    {
        return Err(CoreSbfError::ChildAck);
    }
    let replay_bytes = frame
        .custody_replay
        .try_borrow_data()
        .map_err(|_| CoreSbfError::ChildAck)?;
    if replay_bytes.len() != CUSTODY_REPLAY_BYTES_V1 {
        return Err(CoreSbfError::ChildAck);
    }
    let replay_digest = hash(&replay_bytes).to_bytes();
    let replay = CustodyReplayV1::decode(&replay_bytes).map_err(|_| CoreSbfError::ChildAck)?;
    drop(replay_bytes);
    let receipt = CustodyReceiptV1::decode(&receipt_bytes).map_err(|_| CoreSbfError::ChildAck)?;
    let request_digest = hash(request_bytes).to_bytes();
    receipt
        .verify_for(request, request_digest, replay_digest)
        .map_err(|_| CoreSbfError::ChildAck)?;
    let expected_poststate = custody_poststate(
        request_digest,
        frame.hoard_vault.key.to_bytes(),
        frame.rent_credit.key.to_bytes(),
        request.rent_lamports,
    );
    if replay.open_vault_count != 0
        || replay.next_revision != request.resulting_revision
        || replay.last_request_digest != request_digest
        || replay.last_poststate_commitment != expected_poststate
        || receipt.evidence.poststate_commitment != expected_poststate
    {
        return Err(CoreSbfError::ChildAck);
    }
    Ok(CustodyCloseEvidence {
        digest: hash(&receipt_bytes).to_bytes(),
        refund: receipt.rent_lamports,
        join: CustodyRequestJoin {
            context: request.context,
            realm: request.realm,
            candidate: request.semantic.candidate,
            order: request.semantic.order,
            order_nonce: request.semantic.order_nonce,
            page_index: request.semantic.page_index,
            execution_index: request.semantic.execution_index,
        },
    })
}

#[inline(never)]
fn execute_close_replay(
    program_id: &Pubkey,
    frame: RetirementAccounts<'_, '_>,
    state: CoreState,
    bundle: &dclutch_market_core_codec::RetirementBundleInputV1,
    request_bytes: &[u8],
    parent_digest: [u8; 32],
    expected_join: CustodyRequestJoin,
) -> Result<CustodyTerminalEvidence, CoreSbfError> {
    let request = CustodyRequestV1::decode(request_bytes).map_err(|_| CoreSbfError::Instruction)?;
    require_custody_request(
        program_id,
        frame,
        state,
        bundle,
        request,
        request_bytes,
        OperationV1::CloseReplay,
        CompartmentV1::None,
        bundle.custody_middle_revision,
        bundle.custody_post_revision,
        bundle.custody_close_replay_request_digest,
        parent_digest,
        1,
    )
    .map_err(|_| CoreSbfError::Instruction)?;
    if request.context != expected_join.context
        || request.realm != expected_join.realm
        || request.semantic.candidate != expected_join.candidate
        || request.semantic.order != expected_join.order
        || request.semantic.order_nonce != expected_join.order_nonce
        || request.semantic.page_index != expected_join.page_index
        || request.semantic.execution_index != expected_join.execution_index
    {
        return Err(CoreSbfError::Instruction);
    }
    let credit_after = frame
        .rent_credit
        .lamports()
        .checked_add(request.rent_lamports)
        .ok_or(CoreSbfError::Arithmetic)?;
    invoke_child(
        program_id,
        frame.custody_program,
        frame.close_replay_authority,
        request.release_set,
        request.market,
        request.context,
        request_bytes,
        &[
            (frame.close_replay_authority, false, true),
            (frame.market, false, false),
            (frame.cache, false, false),
            (frame.registry, false, false),
            (frame.core_program, false, false),
            (frame.core_programdata, false, false),
            (frame.realm_raw, false, false),
            (frame.realm_staging, false, false),
            (frame.custody_replay, true, false),
            (frame.rent_credit, true, false),
        ],
    )
    .map_err(|_| CoreSbfError::ChildCpi)?;
    let (producer, receipt_bytes) = get_return_data().ok_or(CoreSbfError::ChildAck)?;
    if producer != *frame.custody_program.key
        || receipt_bytes.len() != CUSTODY_RECEIPT_BYTES_V1
        || frame.custody_replay.owner != &system_program::ID
        || !frame.custody_replay.data_is_empty()
        || frame.custody_replay.lamports() != 0
        || frame.rent_credit.lamports() != credit_after
    {
        return Err(CoreSbfError::ChildAck);
    }
    let receipt = CustodyReceiptV1::decode(&receipt_bytes).map_err(|_| CoreSbfError::ChildAck)?;
    let request_digest = hash(request_bytes).to_bytes();
    receipt
        .verify_for(request, request_digest, hash(&[]).to_bytes())
        .map_err(|_| CoreSbfError::ChildAck)?;
    let expected_poststate = custody_poststate(
        request_digest,
        frame.custody_replay.key.to_bytes(),
        frame.rent_credit.key.to_bytes(),
        request.rent_lamports,
    );
    if receipt.evidence.poststate_commitment != expected_poststate {
        return Err(CoreSbfError::ChildAck);
    }
    Ok(CustodyTerminalEvidence {
        digest: hash(&receipt_bytes).to_bytes(),
        revision: receipt.resulting_revision,
        refund: receipt.rent_lamports,
    })
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn plan_retired_transition(
    request: Request,
    state: CoreState,
    admissions: crate::release::RoleBatchAdmissions,
    expected_core_lamports: u64,
) -> Result<RetiredTransitionPlan, CoreSbfError> {
    let retirement_admissions = admissions.retirement(state)?;
    let mut candidate = state;
    let core_refund = retire(
        request,
        &mut candidate,
        retirement_admissions,
        ClaimsEffectObservation {
            child: complete_child(),
            payout: 0,
            aggregate_empty: true,
        },
        complete_child(),
        complete_child(),
        expected_core_lamports,
        true,
        true,
    )
    .map_err(|_| CoreSbfError::Transition)?;
    let candidate_bytes = candidate.encode().map_err(|_| CoreSbfError::Transition)?;
    let retired_candidate_digest = hashv(&[
        RETIRED_CANDIDATE_DIGEST_DOMAIN_V1.as_slice(),
        candidate_bytes.as_slice(),
    ])
    .to_bytes();
    Ok(RetiredTransitionPlan {
        core_refund,
        candidate_digest: retired_candidate_digest,
    })
}

#[inline(never)]
fn commit_retired(
    program_id: &Pubkey,
    frame: RetirementAccounts<'_, '_>,
    bundle: &RetirementBundleV1,
    bundle_bytes: &[u8],
    evidence: RetirementEvidence,
    rent_before: u64,
    transition: RetiredTransitionPlan,
) -> ProgramResult {
    let bundle_input = bundle.input_ref();
    if evidence.source_digest != bundle_input.source_receipt_digest
        || evidence.source_revision != bundle_input.source_closure_revision
        || evidence.claims_revision != bundle_input.claims_post_revision
        || evidence.custody_revision != bundle_input.custody_post_revision
    {
        return Err(CoreSbfError::ChildAck.into());
    }
    let final_credit = rent_before
        .checked_add(evidence.claims_refund)
        .and_then(|value| value.checked_add(evidence.custody_refund))
        .and_then(|value| value.checked_add(transition.core_refund))
        .ok_or(CoreSbfError::Arithmetic)?;
    let post_resource_digest = hashv(&[
        RETIREMENT_POST_RESOURCE_DIGEST_DOMAIN_V1.as_slice(),
        &[RETIREMENT_ROLE_COUNT_V1],
        &[RETIREMENT_CUSTODY_RECEIPT_COUNT_V1],
        frame.rent_credit.key.as_ref(),
        evidence.source_digest.as_slice(),
        evidence.claims_digest.as_slice(),
        evidence.close_vault_digest.as_slice(),
        evidence.close_replay_digest.as_slice(),
        transition.core_refund.to_le_bytes().as_slice(),
        evidence.claims_refund.to_le_bytes().as_slice(),
        evidence.custody_refund.to_le_bytes().as_slice(),
        final_credit.to_le_bytes().as_slice(),
    ])
    .to_bytes();
    let receipt = RetirementReceiptV1::new(RetirementReceiptInputV1 {
        core_program: program_id.to_bytes(),
        market: bundle_input.market,
        release_set: bundle_input.release_set,
        rent_credit: bundle_input.rent_credit,
        bundle_digest: hash(bundle_bytes).to_bytes(),
        source_receipt_digest: evidence.source_digest,
        claims_receipt_digest: evidence.claims_digest,
        custody_close_vault_receipt_digest: evidence.close_vault_digest,
        custody_close_replay_receipt_digest: evidence.close_replay_digest,
        pre_state_digest: bundle_input.core_prestate_digest,
        retired_candidate_digest: transition.candidate_digest,
        post_resource_digest,
        generation: bundle_input.generation,
        source_closure_revision: evidence.source_revision,
        claims_post_revision: evidence.claims_revision,
        custody_post_revision: evidence.custody_revision,
        core_refund_lamports: transition.core_refund,
        claims_refund_lamports: evidence.claims_refund,
        custody_refund_lamports: evidence.custody_refund,
    })
    .map_err(|_| CoreSbfError::Commit)?;
    receipt
        .verify_for(
            *bundle,
            hash(bundle_bytes).to_bytes(),
            evidence.claims_digest,
            evidence.close_vault_digest,
            evidence.close_replay_digest,
        )
        .map_err(|_| CoreSbfError::Commit)?;
    let receipt_bytes = receipt.to_bytes();
    if receipt_bytes.len() != RETIREMENT_RECEIPT_BYTES_V1 {
        return Err(CoreSbfError::Commit.into());
    }
    close_market(frame, final_credit)?;
    close_lifecycle_credit(program_id, frame, receipt, final_credit)?;
    set_return_data(&receipt_bytes);
    Ok(())
}

#[inline(never)]
fn close_market(frame: RetirementAccounts<'_, '_>, final_credit: u64) -> ProgramResult {
    {
        let mut data = frame
            .market
            .try_borrow_mut_data()
            .map_err(|_| CoreSbfError::Commit)?;
        data.fill(0);
    }
    {
        let mut market_lamports = frame
            .market
            .try_borrow_mut_lamports()
            .map_err(|_| CoreSbfError::Commit)?;
        let mut credit_lamports = frame
            .rent_credit
            .try_borrow_mut_lamports()
            .map_err(|_| CoreSbfError::Commit)?;
        **market_lamports = 0;
        **credit_lamports = final_credit;
    }
    frame.market.resize(0).map_err(|_| CoreSbfError::Commit)?;
    frame.market.assign(&system_program::ID);
    if frame.market.owner != &system_program::ID
        || !frame.market.data_is_empty()
        || frame.market.lamports() != 0
        || frame.rent_credit.lamports() != final_credit
    {
        return Err(CoreSbfError::Commit.into());
    }
    Ok(())
}

#[inline(never)]
fn close_lifecycle_credit(
    program_id: &Pubkey,
    frame: RetirementAccounts<'_, '_>,
    receipt: RetirementReceiptV1,
    final_credit: u64,
) -> ProgramResult {
    let receipt_input = receipt.input();
    let credit_id = LifecycleAccountIdV2::new(frame.rent_credit.key.to_bytes())
        .map_err(|_| CoreSbfError::RentCredit)?;
    let seeds =
        LifecycleRentCoreCloseAuthoritySeedsV2::new(credit_id, receipt_input.post_resource_digest)
            .map_err(|_| CoreSbfError::RentCredit)?;
    let credit = seeds.credit().to_bytes();
    let post_resource_digest = seeds.post_resource_digest();
    let (expected_authority, bump) = Pubkey::find_program_address(
        &[
            seeds.domain(),
            credit.as_slice(),
            post_resource_digest.as_slice(),
        ],
        program_id,
    );
    if expected_authority != *frame.rent_close_authority.key {
        return Err(CoreSbfError::CallerAuthority.into());
    }
    let wallet_before = frame.refund_wallet.lamports();
    let wallet_after = wallet_before
        .checked_add(final_credit)
        .ok_or(CoreSbfError::Arithmetic)?;
    let request_bytes = CloseLifecycleRentCreditV2::new(receipt).to_bytes();
    let instruction = Instruction {
        program_id: *frame.rent_program.key,
        accounts: Vec::from([
            AccountMeta::new(*frame.rent_credit.key, false),
            AccountMeta::new(*frame.refund_wallet.key, false),
            AccountMeta::new_readonly(*frame.cache.key, false),
            AccountMeta::new_readonly(*frame.registry.key, false),
            AccountMeta::new_readonly(*frame.core_program.key, false),
            AccountMeta::new_readonly(*frame.core_programdata.key, false),
            AccountMeta::new_readonly(*frame.rent_close_authority.key, true),
        ]),
        data: request_bytes.to_vec(),
    };
    let bump_seed = [bump];
    invoke_signed(
        &instruction,
        &[
            frame.rent_credit.clone(),
            frame.refund_wallet.clone(),
            frame.cache.clone(),
            frame.registry.clone(),
            frame.core_program.clone(),
            frame.core_programdata.clone(),
            frame.rent_close_authority.clone(),
            frame.rent_program.clone(),
        ],
        &[&[
            seeds.domain(),
            credit.as_slice(),
            post_resource_digest.as_slice(),
            &bump_seed,
        ]],
    )
    .map_err(|_| CoreSbfError::ChildCpi)?;
    let (producer, return_bytes) = get_return_data().ok_or(CoreSbfError::ChildAck)?;
    if producer != *frame.rent_program.key
        || return_bytes.len() != LIFECYCLE_RENT_CLOSE_RECEIPT_BYTES_V2
        || frame.rent_credit.owner != &system_program::ID
        || !frame.rent_credit.data_is_empty()
        || frame.rent_credit.lamports() != 0
        || frame.refund_wallet.lamports() != wallet_after
    {
        return Err(CoreSbfError::ChildAck.into());
    }
    let rent_receipt =
        LifecycleRentCloseReceiptV2::decode(&return_bytes).map_err(|_| CoreSbfError::ChildAck)?;
    let rent_input = rent_receipt.input();
    if rent_input.credit.to_bytes() != receipt_input.rent_credit
        || rent_input.refund_wallet.to_bytes() != frame.refund_wallet.key.to_bytes()
        || rent_input.market.to_bytes() != receipt_input.market
        || rent_input.release_set.to_bytes() != receipt_input.release_set
        || rent_input.post_resource_digest != receipt_input.post_resource_digest
        || rent_input.generation != receipt_input.generation
        || rent_input.closed_lamports != final_credit
    {
        return Err(CoreSbfError::ChildAck.into());
    }
    Ok(())
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn invoke_child<'info>(
    program_id: &Pubkey,
    child_program: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    release_set: [u8; 32],
    market: [u8; 32],
    context: [u8; 32],
    request_bytes: &[u8],
    account_projection: &[(&AccountInfo<'info>, bool, bool)],
) -> ProgramResult {
    let digest = hash(request_bytes).to_bytes();
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        release_set,
        market,
        ExecutionRoleV1::Core,
        context,
        digest,
    )
    .map_err(|_| CoreSbfError::CallerAuthority)?;
    let (expected, bump) = Pubkey::find_program_address(&seeds.as_slices(), program_id);
    if expected != *authority.key {
        return Err(CoreSbfError::CallerAuthority.into());
    }
    let mut metas = Vec::with_capacity(account_projection.len());
    let mut infos = Vec::with_capacity(account_projection.len().saturating_add(1));
    for (account, writable, signer) in account_projection.iter().copied() {
        metas.push(if writable {
            AccountMeta::new(*account.key, signer)
        } else {
            AccountMeta::new_readonly(*account.key, signer)
        });
        infos.push(account.clone());
    }
    infos.push(child_program.clone());
    let instruction = Instruction {
        program_id: *child_program.key,
        accounts: metas,
        data: request_bytes.to_vec(),
    };
    let bump_seed = [bump];
    let [
        domain,
        release,
        market_seed,
        role,
        context_seed,
        request_digest,
    ] = seeds.as_slices();
    invoke_signed(
        &instruction,
        &infos,
        &[&[
            domain,
            release,
            market_seed,
            role,
            context_seed,
            request_digest,
            &bump_seed,
        ]],
    )
    .map_err(|_| CoreSbfError::ChildCpi.into())
}

fn custody_poststate(
    request_digest: [u8; 32],
    source: [u8; 32],
    destination: [u8; 32],
    rent_lamports: u64,
) -> [u8; 32] {
    hashv(&[
        CUSTODY_POSTSTATE_DOMAIN_V1,
        request_digest.as_slice(),
        source.as_slice(),
        destination.as_slice(),
        0_u64.to_le_bytes().as_slice(),
        0_u64.to_le_bytes().as_slice(),
        0_u64.to_le_bytes().as_slice(),
        0_u64.to_le_bytes().as_slice(),
        rent_lamports.to_le_bytes().as_slice(),
    ])
    .to_bytes()
}

const fn complete_child() -> ChildEffectObservation {
    ChildEffectObservation {
        exact_request_authenticated: true,
        exact_receipt_authenticated: true,
        post_resource_authenticated: true,
    }
}

#[inline(never)]
fn authenticate_privileges(
    program_id: &Pubkey,
    frame: RetirementAccounts<'_, '_>,
) -> ProgramResult {
    if frame.market.key == frame.rent_credit.key
        || frame.market.key == frame.claims_aggregate.key
        || frame.market.key == frame.custody_replay.key
        || frame.market.key == frame.hoard_vault.key
        || frame.rent_credit.key == frame.claims_aggregate.key
        || frame.rent_credit.key == frame.custody_replay.key
        || frame.rent_credit.key == frame.hoard_vault.key
        || frame.claims_aggregate.key == frame.custody_replay.key
        || frame.claims_aggregate.key == frame.hoard_vault.key
        || frame.custody_replay.key == frame.hoard_vault.key
        || !frame.market.is_writable
        || frame.market.is_signer
        || !frame.rent_credit.is_writable
        || frame.rent_credit.is_signer
        || !frame.claims_aggregate.is_writable
        || frame.claims_aggregate.is_signer
        || !frame.custody_replay.is_writable
        || frame.custody_replay.is_signer
        || !frame.hoard_vault.is_writable
        || frame.hoard_vault.is_signer
        || !frame.refund_wallet.is_writable
        || frame.refund_wallet.is_signer
        || frame.refund_wallet.executable
        || frame.core_program.key != program_id
        || !frame.core_program.executable
        || !frame.claims_program.executable
        || !frame.resolution_program.executable
        || !frame.custody_program.executable
        || !frame.registry.executable
        || !frame.rent_program.executable
        || !frame.token_program.executable
        || frame.rent.key != &sysvar::rent::ID
        || frame.rent.is_writable
        || frame.rent.is_signer
        || frame.rent.executable
    {
        return Err(CoreSbfError::AccountFrame.into());
    }
    for account in [
        frame.cache,
        frame.core_programdata,
        frame.claims_programdata,
        frame.resolution_programdata,
        frame.custody_programdata,
        frame.source_receipt,
        frame.custody_authority,
        frame.collateral_mint,
        frame.realm_raw,
        frame.realm_staging,
        frame.claims_authority,
        frame.close_vault_authority,
        frame.close_replay_authority,
        frame.infrastructure_profile,
        frame.registry_artifact_raw,
        frame.registry_artifact_staging,
        frame.registry_programdata,
        frame.rent_artifact_raw,
        frame.rent_artifact_staging,
        frame.rent_programdata,
        frame.rent_close_authority,
    ] {
        if account.is_writable || account.is_signer || account.executable {
            return Err(CoreSbfError::AccountFrame.into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retirement_frame_and_instruction_width_are_exact() {
        assert_eq!(RETIREMENT_ACCOUNT_COUNT_V1, 35);
        assert_eq!(RETIREMENT_INSTRUCTION_BYTES_V1, 2_152);
        assert!(RetirementAccounts::parse(&[]).is_err());
    }
}
