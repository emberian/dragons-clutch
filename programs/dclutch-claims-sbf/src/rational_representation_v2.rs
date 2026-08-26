//! Claims-owned physical adapter for exact rational representations.
//!
//! Immutable finalized descriptor and graph records own interpretation. Token
//! programs own Mint supplies and holder balances. The canonical Claims
//! economic kernel owns native and materialized quantities. This module owns
//! only one per-descriptor/actor replay revision and commits it after every
//! Claims, Token, and Custody postcondition has passed.

use dclutch_claims_svm::{
    affine_batch_v2::{AffineBatchPlanV2, AffineBatchReceiptV2},
    lbv2_terminal_v2::{Lbv2TerminalRedeemReceiptV2, Lbv2TerminalRedeemRequestV2},
    protocol_position_v2::ProtocolPositionSeedsV2,
};
use dclutch_custody_contract::{CustodyReceiptV1, CustodyRequestV1};
use dclutch_liability_basis_v2_kernel::product_claims::{
    AdmittedBasisV2, ContentIdV2, LinkedBasisRecordV2,
};
use dclutch_market_core_codec::Phase as CorePhase;
use dclutch_product_runtime_v2_svm_reader::{FinalizedRecordFrameV2, ProductRuntimeFrameV2};
use dclutch_rational_representation_v2_contract::{
    AffineBatchContextV2, CompletionEvidenceV2, PreparedRepresentationV2,
    RATIONAL_ASSET_ACCOUNT_COUNT_V2, RATIONAL_BASE_ACCOUNT_COUNT_V2,
    RATIONAL_TERMINAL_ACCOUNT_COUNT_V2, RationalReplayV2, RepresentationActionV2,
    RepresentationRequestV2, TokenEffectStyleV2, finalize, prepare,
};
use dclutch_rational_representation_v2_kernel::{
    ContentAdmissionV2, CoordinateObservation, DescriptorAdmissionV2,
    REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3, REPRESENTATION_GRAPH_SCHEMA_RELEASE_ID_V2,
    RepresentationDescriptorV2, RepresentationGraphV2, SCALAR_BYTES, STRUCTURED_HEADER_BYTES,
    STRUCTURED_VECTOR_COUNT, StructuredProjectionHeaderV2, StructuredProjectionV2,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_token_svm::{
    ACCOUNT_BYTES, AccountState, COption, MINT_BYTES, Mint, TokenAccount, TokenProgram,
};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    program::{invoke, invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::{allocate, assign};
use spl_associated_token_account_interface::address::get_associated_token_address_with_program_id;
use spl_token_2022_interface::instruction as token_instruction;

use super::{ClaimsSbfError, reauthenticate};
use crate::{
    affine_batch_v2::{
        AFFINE_BATCH_FIXED_ACCOUNT_COUNT_V2, AuthenticatedAffineParentV2,
        authenticate_runtime_product_basis_core_v2, execute_parent_authenticated,
    },
    liability_basis_v2::{
        AuthenticatedLbv2TerminalParentV2, LIABILITY_BASIS_ACCOUNT_COUNT_V2,
        LIABILITY_BASIS_MARKET_SEED_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, MarketViewV2,
        PositionViewV2, execute_parent_authenticated_terminal_v2, read_vector,
    },
};

pub use dclutch_rational_representation_v2_contract::{
    RATIONAL_CLAIMS_CUSTODY_OWNER_SEED_V2, RATIONAL_REPLAY_BYTES_V2, RATIONAL_REPLAY_MAGIC_V2,
    RATIONAL_REPLAY_SEED_V2, RATIONAL_REPLAY_VERSION_V2, RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2,
    RATIONAL_SHARD_MINT_SEED_V2,
};
const CALLER_AUTHORITY: usize = 0;
const CALLER_PROGRAM: usize = 1;
const CALLER_PROGRAMDATA: usize = 2;
const ACTOR: usize = 3;
const REPRESENTATION_AUTHORITY: usize = 4;
const DESCRIPTOR_RAW: usize = 5;
const DESCRIPTOR_STAGING: usize = 6;
const GRAPH_RAW: usize = 7;
const GRAPH_STAGING: usize = 8;
const RENT_SYSVAR: usize = 9;
const SYSTEM_PROGRAM: usize = 10;
const REPLAY: usize = 11;
const CLAIMS_AGGREGATE: usize = 12;
const ACTIVATION_CACHE: usize = 13;
const CLAIMS_PROGRAM: usize = 14;
const CLAIMS_PROGRAMDATA: usize = 15;
const REGISTRY_PROGRAM: usize = 16;
const CORE_MARKET: usize = 17;
const CORE_PROGRAM: usize = 18;
const CORE_PROGRAMDATA: usize = 19;
const RECEIPT_MINT: usize = 20;
const ACTOR_RECEIPT_ACCOUNT: usize = 21;
const REPRESENTATION_TOKEN_PROGRAM: usize = 22;
const ACTOR_CLAIMS_POSITION: usize = 23;
const LINKED_BASIS_RECORD: usize = 24;
const LINKED_BASIS_STAGING: usize = 25;
const PRODUCT_RECORD: usize = 26;
const PRODUCT_STAGING: usize = 27;
const RESULT_DOMAIN_RECORD: usize = 28;
const RESULT_DOMAIN_STAGING: usize = 29;
const PORTFOLIO_RECORD: usize = 30;
const PORTFOLIO_STAGING: usize = 31;

const ASSET_POSITION: usize = 0;
const ASSET_SHARD_MINT: usize = 1;
const ASSET_ACTOR_TOKEN: usize = 2;
const ASSET_STRUCTURED_TOKEN: usize = 3;

const TERMINAL_CALLER_AUTHORITY: usize = 0;
const TERMINAL_CUSTODY_PROGRAM: usize = 1;
const TERMINAL_CUSTODY_PROGRAMDATA: usize = 2;
const TERMINAL_COORDINATE: usize = 3;
const TERMINAL_COORDINATE_STAGING: usize = 4;
const TERMINAL_REALM: usize = 5;
const TERMINAL_REALM_STAGING: usize = 6;
const TERMINAL_CUSTODY_REPLAY: usize = 7;
const TERMINAL_COLLATERAL_MINT: usize = 8;
const TERMINAL_HOARD: usize = 9;
const TERMINAL_RECIPIENT: usize = 10;
const TERMINAL_CUSTODY_AUTHORITY: usize = 11;
const TERMINAL_TOKEN_PROGRAM: usize = 12;

#[derive(Clone, Copy)]
struct BaseAccounts<'accounts, 'info> {
    caller_authority: &'accounts AccountInfo<'info>,
    caller_program: &'accounts AccountInfo<'info>,
    caller_programdata: &'accounts AccountInfo<'info>,
    actor: &'accounts AccountInfo<'info>,
    representation_authority: &'accounts AccountInfo<'info>,
    descriptor_raw: &'accounts AccountInfo<'info>,
    descriptor_staging: &'accounts AccountInfo<'info>,
    graph_raw: &'accounts AccountInfo<'info>,
    graph_staging: &'accounts AccountInfo<'info>,
    rent: &'accounts AccountInfo<'info>,
    system: &'accounts AccountInfo<'info>,
    replay: &'accounts AccountInfo<'info>,
    aggregate: &'accounts AccountInfo<'info>,
    cache: &'accounts AccountInfo<'info>,
    claims_program: &'accounts AccountInfo<'info>,
    claims_programdata: &'accounts AccountInfo<'info>,
    registry: &'accounts AccountInfo<'info>,
    core_market: &'accounts AccountInfo<'info>,
    core_program: &'accounts AccountInfo<'info>,
    core_programdata: &'accounts AccountInfo<'info>,
    receipt_mint: &'accounts AccountInfo<'info>,
    actor_receipt: &'accounts AccountInfo<'info>,
    token_program: &'accounts AccountInfo<'info>,
    actor_position: &'accounts AccountInfo<'info>,
    linked_basis_record: &'accounts AccountInfo<'info>,
    linked_basis_staging: &'accounts AccountInfo<'info>,
    product_record: &'accounts AccountInfo<'info>,
    product_staging: &'accounts AccountInfo<'info>,
    result_domain_record: &'accounts AccountInfo<'info>,
    result_domain_staging: &'accounts AccountInfo<'info>,
    portfolio_record: &'accounts AccountInfo<'info>,
    portfolio_staging: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> BaseAccounts<'accounts, 'info> {
    fn parse(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        Ok(Self {
            caller_authority: account(accounts, CALLER_AUTHORITY)?,
            caller_program: account(accounts, CALLER_PROGRAM)?,
            caller_programdata: account(accounts, CALLER_PROGRAMDATA)?,
            actor: account(accounts, ACTOR)?,
            representation_authority: account(accounts, REPRESENTATION_AUTHORITY)?,
            descriptor_raw: account(accounts, DESCRIPTOR_RAW)?,
            descriptor_staging: account(accounts, DESCRIPTOR_STAGING)?,
            graph_raw: account(accounts, GRAPH_RAW)?,
            graph_staging: account(accounts, GRAPH_STAGING)?,
            rent: account(accounts, RENT_SYSVAR)?,
            system: account(accounts, SYSTEM_PROGRAM)?,
            replay: account(accounts, REPLAY)?,
            aggregate: account(accounts, CLAIMS_AGGREGATE)?,
            cache: account(accounts, ACTIVATION_CACHE)?,
            claims_program: account(accounts, CLAIMS_PROGRAM)?,
            claims_programdata: account(accounts, CLAIMS_PROGRAMDATA)?,
            registry: account(accounts, REGISTRY_PROGRAM)?,
            core_market: account(accounts, CORE_MARKET)?,
            core_program: account(accounts, CORE_PROGRAM)?,
            core_programdata: account(accounts, CORE_PROGRAMDATA)?,
            receipt_mint: account(accounts, RECEIPT_MINT)?,
            actor_receipt: account(accounts, ACTOR_RECEIPT_ACCOUNT)?,
            token_program: account(accounts, REPRESENTATION_TOKEN_PROGRAM)?,
            actor_position: account(accounts, ACTOR_CLAIMS_POSITION)?,
            linked_basis_record: account(accounts, LINKED_BASIS_RECORD)?,
            linked_basis_staging: account(accounts, LINKED_BASIS_STAGING)?,
            product_record: account(accounts, PRODUCT_RECORD)?,
            product_staging: account(accounts, PRODUCT_STAGING)?,
            result_domain_record: account(accounts, RESULT_DOMAIN_RECORD)?,
            result_domain_staging: account(accounts, RESULT_DOMAIN_STAGING)?,
            portfolio_record: account(accounts, PORTFOLIO_RECORD)?,
            portfolio_staging: account(accounts, PORTFOLIO_STAGING)?,
        })
    }
}

#[derive(Clone, Copy)]
struct AssetAccounts<'accounts, 'info> {
    position: &'accounts AccountInfo<'info>,
    mint: &'accounts AccountInfo<'info>,
    actor_token: &'accounts AccountInfo<'info>,
    structured_token: &'accounts AccountInfo<'info>,
}

#[derive(Clone, Copy)]
struct TerminalAccounts<'accounts, 'info> {
    caller_authority: &'accounts AccountInfo<'info>,
    custody_program: &'accounts AccountInfo<'info>,
    custody_programdata: &'accounts AccountInfo<'info>,
    coordinate: &'accounts AccountInfo<'info>,
    coordinate_staging: &'accounts AccountInfo<'info>,
    realm: &'accounts AccountInfo<'info>,
    realm_staging: &'accounts AccountInfo<'info>,
    replay: &'accounts AccountInfo<'info>,
    collateral_mint: &'accounts AccountInfo<'info>,
    hoard: &'accounts AccountInfo<'info>,
    recipient: &'accounts AccountInfo<'info>,
    custody_authority: &'accounts AccountInfo<'info>,
    token_program: &'accounts AccountInfo<'info>,
}

struct CustodyEvidence {
    request: Box<CustodyRequestV1>,
    request_digest: [u8; 32],
    receipt: Box<CustodyReceiptV1>,
    receipt_digest: [u8; 32],
    replay_digest: [u8; 32],
}

struct ClaimsEvidence {
    plan_digest: [u8; 32],
    packet: Vec<u8>,
    context: Option<AffineBatchContextV2>,
    receipt: Option<AffineBatchReceiptV2>,
    terminal_request: Option<Box<Lbv2TerminalRedeemRequestV2>>,
    terminal_request_digest: [u8; 32],
    terminal_receipt: Option<Box<Lbv2TerminalRedeemReceiptV2>>,
    custody: Option<Box<CustodyEvidence>>,
}

/// Execute one exact RationalRepresentationV2 request.
#[inline(never)]
pub(crate) fn process(
    program_id: &Pubkey,
    account_infos: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let request = RepresentationRequestV2::decode(instruction_data)
        .map_err(|_| ClaimsSbfError::Instruction)?;
    let base = BaseAccounts::parse(account_infos)?;
    let request_digest = hash(instruction_data).to_bytes();
    authenticate_base(program_id, account_infos, base, request, request_digest)?;

    prepare_and_execute(program_id, account_infos, base, request, request_digest)
}

#[inline(never)]
fn prepare_and_execute<'accounts, 'info>(
    program_id: &Pubkey,
    account_infos: &'accounts [AccountInfo<'info>],
    base: BaseAccounts<'accounts, 'info>,
    request: RepresentationRequestV2<'_>,
    request_digest: [u8; 32],
) -> Result<(), ProgramError> {
    let header = request.header();
    let descriptor_data = base
        .descriptor_raw
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    authenticate_finalized_record(
        base,
        base.descriptor_raw,
        base.descriptor_staging,
        REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
        header.descriptor_id,
        &descriptor_data,
    )?;
    let descriptor = RepresentationDescriptorV2::decode(
        &descriptor_data,
        DescriptorAdmissionV2 {
            selected_descriptor_id: header.descriptor_id,
            finalized_descriptor_id: header.descriptor_id,
            recomputed_descriptor_digest: request_digest_of(&descriptor_data),
            finalized_descriptor_digest: header.descriptor_id,
            record_authenticated: true,
            derived_representation_authority: header.representation_authority,
            authority_derivation_authenticated: true,
        },
    )
    .map_err(|_| ClaimsSbfError::Representation)?;

    let graph_data = base
        .graph_raw
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    authenticate_finalized_record(
        base,
        base.graph_raw,
        base.graph_staging,
        REPRESENTATION_GRAPH_SCHEMA_RELEASE_ID_V2,
        descriptor.graph_digest(),
        &graph_data,
    )?;
    let graph = RepresentationGraphV2::decode(
        &graph_data,
        ContentAdmissionV2 {
            selected_graph_id: header.graph_id,
            finalized_graph_id: header.graph_id,
            recomputed_graph_digest: descriptor.graph_digest(),
            finalized_graph_digest: descriptor.graph_digest(),
            record_authenticated: true,
        },
    )
    .map_err(|_| ClaimsSbfError::Representation)?;

    let (_core_generation, admitted_basis) =
        authenticate_core_and_economics(program_id, base, request)?;
    let replay_fresh = authenticate_or_allocate_replay(program_id, base, request)?;
    let projection_bytes = build_projection(program_id, account_infos, base, request, descriptor)?;
    let projection = StructuredProjectionV2::decode(&projection_bytes)
        .map_err(|_| ClaimsSbfError::Representation)?;
    let prepared = prepare(request, descriptor, projection, graph)
        .map_err(|_| ClaimsSbfError::Representation)?;

    execute_prepared(
        program_id,
        account_infos,
        base,
        prepared,
        request_digest,
        admitted_basis,
        replay_fresh,
    )
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn execute_prepared<'accounts, 'info>(
    program_id: &Pubkey,
    account_infos: &'accounts [AccountInfo<'info>],
    base: BaseAccounts<'accounts, 'info>,
    prepared: PreparedRepresentationV2<'_>,
    request_digest: [u8; 32],
    admitted_basis: AdmittedBasisV2,
    replay_fresh: bool,
) -> Result<(), ProgramError> {
    let token_effect_digest = token_effect_digest(prepared)?;
    execute_token_effects(program_id, account_infos, base, prepared)?;
    let claims = execute_claims_if_any(
        program_id,
        account_infos,
        base,
        prepared,
        request_digest,
        admitted_basis,
    )?;

    finalize_execution(
        program_id,
        account_infos,
        base,
        prepared,
        request_digest,
        token_effect_digest,
        &claims,
        claims.custody.as_deref(),
        replay_fresh,
    )
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn finalize_execution(
    program_id: &Pubkey,
    account_infos: &[AccountInfo<'_>],
    base: BaseAccounts<'_, '_>,
    prepared: PreparedRepresentationV2<'_>,
    request_digest: [u8; 32],
    token_effect_digest: [u8; 32],
    claims: &ClaimsEvidence,
    custody: Option<&CustodyEvidence>,
    replay_fresh: bool,
) -> Result<(), ProgramError> {
    let request = prepared.request();
    let (post_asset_observations, post_receipt_supply) =
        post_token_observations(account_infos, base, request)?;
    let post_resource_digest = post_resource_digest(account_infos, base, request, custody)?;
    let affine_packet = if claims.packet.is_empty() {
        None
    } else {
        Some(AffineBatchPlanV2::decode(&claims.packet).map_err(|_| ClaimsSbfError::Receipt)?)
    };
    let evidence = CompletionEvidenceV2 {
        request_digest,
        representation_program: program_id.to_bytes(),
        claims_program: program_id.to_bytes(),
        affine_packet_digest: claims.plan_digest,
        affine_packet,
        affine_context: claims.context,
        affine_receipt: claims.receipt,
        terminal_request: claims.terminal_request.as_deref(),
        terminal_request_digest: claims.terminal_request_digest,
        terminal_receipt: claims.terminal_receipt.as_deref(),
        token_effect_digest,
        post_receipt_supply,
        post_asset_observations: &post_asset_observations,
        custody_request: custody.map(|value| value.request.as_ref()),
        custody_request_digest: custody.map_or([0; 32], |value| value.request_digest),
        custody_receipt: custody.map(|value| value.receipt.as_ref()),
        custody_receipt_digest: custody.map_or([0; 32], |value| value.receipt_digest),
        custody_replay_digest: custody.map_or([0; 32], |value| value.replay_digest),
        post_resource_digest,
    };
    let receipt_bytes = build_receipt_bytes(prepared, evidence)?;
    commit_replay(base, request, replay_fresh)?;
    set_return_data(&receipt_bytes);
    Ok(())
}

#[inline(never)]
fn build_receipt_bytes(
    prepared: PreparedRepresentationV2<'_>,
    evidence: CompletionEvidenceV2<'_>,
) -> Result<Vec<u8>, ProgramError> {
    let receipt = finalize(prepared, evidence).map_err(|_| ClaimsSbfError::Receipt)?;
    receipt
        .to_bytes()
        .map(|bytes| bytes.to_vec())
        .map_err(|_| ClaimsSbfError::Receipt.into())
}

fn authenticate_base(
    program_id: &Pubkey,
    account_infos: &[AccountInfo<'_>],
    base: BaseAccounts<'_, '_>,
    request: RepresentationRequestV2<'_>,
    request_digest: [u8; 32],
) -> Result<(), ProgramError> {
    let header = request.header();
    let asset_accounts = usize::try_from(header.asset_count)
        .ok()
        .and_then(|count| count.checked_mul(RATIONAL_ASSET_ACCOUNT_COUNT_V2))
        .ok_or(ClaimsSbfError::Accounts)?;
    let terminal_offset = RATIONAL_BASE_ACCOUNT_COUNT_V2
        .checked_add(asset_accounts)
        .ok_or(ClaimsSbfError::Accounts)?;
    if account_infos.len() != terminal_offset
        && account_infos.len()
            != terminal_offset
                .checked_add(RATIONAL_TERMINAL_ACCOUNT_COUNT_V2)
                .ok_or(ClaimsSbfError::Accounts)?
    {
        return Err(ClaimsSbfError::Accounts.into());
    }
    let role = caller_role(header.caller_role);
    let caller_seeds = CallerAuthoritySeedsV1::new(
        dclutch_core_contract::ContentId::new(header.release_set)
            .map_err(|_| ClaimsSbfError::Identity)?,
        header.market,
        role,
        header.parent_context,
        request_digest,
    )
    .map_err(|_| ClaimsSbfError::Authority)?;
    let expected_caller =
        Pubkey::find_program_address(&caller_seeds.as_slices(), base.caller_program.key).0;
    let expected_representation = Pubkey::find_program_address(
        &[
            RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2,
            &header.descriptor_id,
        ],
        program_id,
    )
    .0;
    if !base.caller_authority.is_signer
        || base.caller_authority.is_writable
        || base.caller_authority.key != &expected_caller
        || !base.caller_program.executable
        || base.caller_program.is_writable
        || base.caller_program.is_signer
        || base.caller_programdata.is_writable
        || base.caller_programdata.is_signer
        || !base.actor.is_signer
        || base.actor.is_writable
        || base.actor.key.to_bytes() != header.actor
        || base.representation_authority.key != &expected_representation
        || base.representation_authority.is_writable
        || base.representation_authority.is_signer
        || base.descriptor_raw.is_writable
        || base.descriptor_raw.is_signer
        || base.descriptor_staging.is_writable
        || base.descriptor_staging.is_signer
        || base.graph_raw.is_writable
        || base.graph_raw.is_signer
        || base.graph_staging.is_writable
        || base.graph_staging.is_signer
        || base.rent.key != &sysvar::rent::ID
        || base.rent.is_writable
        || base.rent.is_signer
        || base.system.key != &system_program::ID
        || !base.system.executable
        || base.system.is_writable
        || base.system.is_signer
        || !base.replay.is_writable
        || base.replay.is_signer
        || base.aggregate.is_writable != header.action.uses_claims()
        || base.aggregate.is_signer
        || base.aggregate.executable
        || base.cache.is_writable
        || base.cache.is_signer
        || base.claims_program.key != program_id
        || !base.claims_program.executable
        || base.claims_program.is_writable
        || base.claims_program.is_signer
        || base.claims_programdata.is_writable
        || base.claims_programdata.is_signer
        || !base.registry.executable
        || base.registry.is_writable
        || base.registry.is_signer
        || base.core_market.is_writable
        || base.core_market.is_signer
        || !base.core_program.executable
        || base.core_program.is_writable
        || base.core_program.is_signer
        || base.core_programdata.is_writable
        || base.core_programdata.is_signer
        || !base.token_program.executable
        || base.token_program.is_writable
        || base.token_program.is_signer
        || base.token_program.key.to_bytes() != header.token_program
        || TokenProgram::parse(header.token_program).is_err()
    {
        return Err(ClaimsSbfError::Accounts.into());
    }
    for immutable in [
        base.linked_basis_record,
        base.linked_basis_staging,
        base.product_record,
        base.product_staging,
        base.result_domain_record,
        base.result_domain_staging,
        base.portfolio_record,
        base.portfolio_staging,
    ] {
        if immutable.is_writable || immutable.is_signer || immutable.executable {
            return Err(ClaimsSbfError::Accounts.into());
        }
    }
    let actor_position_present = matches!(
        header.action,
        RepresentationActionV2::Denominate | RepresentationActionV2::Reconstitute
    );
    if actor_position_present {
        let seeds = ProtocolPositionSeedsV2::new(base.aggregate.key.to_bytes(), header.actor)
            .map_err(|_| ClaimsSbfError::Identity)?;
        let expected = Pubkey::find_program_address(&seeds.as_slices(), program_id).0;
        if base.actor_position.key != &expected
            || base.actor_position.owner != program_id
            || !base.actor_position.is_writable
            || base.actor_position.is_signer
            || base.actor_position.executable
        {
            return Err(ClaimsSbfError::Accounts.into());
        }
    } else if base.actor_position.key != base.claims_program.key
        || base.actor_position.is_writable
        || !base.actor_position.executable
    {
        return Err(ClaimsSbfError::Accounts.into());
    }
    let receipt_writable = matches!(
        header.action,
        RepresentationActionV2::IssueStructured | RepresentationActionV2::UnwrapStructured
    );
    if base.receipt_mint.key.to_bytes() != header.receipt_mint
        || base.receipt_mint.owner != base.token_program.key
        || base.receipt_mint.is_writable != receipt_writable
        || base.receipt_mint.is_signer
        || (receipt_writable
            && (base.actor_receipt.key.to_bytes() != header.receipt_account
                || base.actor_receipt.owner != base.token_program.key
                || !base.actor_receipt.is_writable
                || base.actor_receipt.is_signer))
        || (!receipt_writable
            && (base.actor_receipt.key != base.claims_program.key
                || base.actor_receipt.is_writable
                || !base.actor_receipt.executable))
    {
        return Err(ClaimsSbfError::Accounts.into());
    }
    Ok(())
}

fn authenticate_finalized_record(
    base: BaseAccounts<'_, '_>,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    schema: [u8; 32],
    expected_digest: [u8; 32],
    bytes: &[u8],
) -> Result<(), ProgramError> {
    let rent = Rent::from_account_info(base.rent).map_err(|_| ClaimsSbfError::Accounts)?;
    authenticate_finalized_rational_record(
        base.registry.key,
        &rent,
        raw,
        staging,
        schema,
        expected_digest,
        bytes,
    )
}

/// Authenticate one immutable Registry record used by RationalRepresentationV2.
///
/// This is the sole in-program finalized-record reader shared by the request
/// executor and its lifecycle route; callers still own semantic decoding.
pub(crate) fn authenticate_finalized_rational_record(
    registry: &Pubkey,
    rent: &Rent,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    schema: [u8; 32],
    expected_digest: [u8; 32],
    bytes: &[u8],
) -> Result<(), ProgramError> {
    let raw_key = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema, &expected_digest],
        registry,
    )
    .0;
    let staging_key = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &schema, &expected_digest],
        registry,
    )
    .0;
    if raw.owner != registry
        || raw.key != &raw_key
        || raw.executable
        || raw.is_writable
        || raw.is_signer
        || hash(bytes).to_bytes() != expected_digest
        || !rent.is_exempt(raw.lamports(), bytes.len())
        || staging.key != &staging_key
        || staging.owner != &system_program::ID
        || staging.data_len() != 0
        || staging.executable
        || staging.is_writable
        || staging.is_signer
    {
        return Err(ClaimsSbfError::Representation.into());
    }
    Ok(())
}

fn authenticate_core_and_economics(
    program_id: &Pubkey,
    base: BaseAccounts<'_, '_>,
    request: RepresentationRequestV2<'_>,
) -> Result<(u64, AdmittedBasisV2), ProgramError> {
    let header = request.header();
    let caller = reauthenticate(
        base.registry,
        base.cache,
        caller_role(header.caller_role),
        base.caller_program,
        base.caller_programdata,
    )?;
    if caller.execution_release_set_id().as_bytes() != &header.release_set {
        return Err(ClaimsSbfError::Release.into());
    }
    // A positive terminal payout crosses the canonical Custody boundary.
    // Custody independently reauthenticates Claims, Custody, and Core against
    // this release set and its typed receipt is required before either Claims
    // state or Rational replay commits. Repeating Claims/Core Registry CPIs in
    // this enclosing route would add no authority and exceeds the transaction
    // CU limit. Open-only paths have no such child proof and retain both checks.
    if header.action != RepresentationActionV2::RedeemTerminal {
        for (role, program, programdata) in [
            (
                ExecutionRoleV1::Claims,
                base.claims_program,
                base.claims_programdata,
            ),
            (
                ExecutionRoleV1::Core,
                base.core_program,
                base.core_programdata,
            ),
        ] {
            let receipt = reauthenticate(base.registry, base.cache, role, program, programdata)?;
            if receipt.execution_release_set_id().as_bytes() != &header.release_set {
                return Err(ClaimsSbfError::Release.into());
            }
        }
    }
    let aggregate = base
        .aggregate
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let market = MarketViewV2::decode(&aggregate).map_err(|_| ClaimsSbfError::Economic)?;
    let expected_market = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, header.market.as_slice()],
        program_id,
    )
    .0;
    if base.aggregate.owner != program_id
        || base.aggregate.key != &expected_market
        || market.logical_market != header.market
        || market.release_set != header.release_set
        || market.registry_program != base.registry.key.to_bytes()
        || market.claim_count != header.outcome_count
        || market.generation != header.generation
        || (header.action.uses_claims()
            && market.revision != header.expected_claims_market_revision)
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    if matches!(
        header.action,
        RepresentationActionV2::Denominate | RepresentationActionV2::Reconstitute
    ) {
        let actor = base
            .actor_position
            .try_borrow_data()
            .map_err(|_| ClaimsSbfError::Accounts)?;
        let position = PositionViewV2::decode(&actor).map_err(|_| ClaimsSbfError::Economic)?;
        if position.market_account != base.aggregate.key.to_bytes()
            || position.owner != header.actor
            || position.basis_id != market.basis_id
            || position.claim_count != market.claim_count
            || position.revision != header.expected_actor_position_revision
        {
            return Err(ClaimsSbfError::Identity.into());
        }
    }
    drop(aggregate);
    let product_digest = {
        let bytes = base
            .product_record
            .try_borrow_data()
            .map_err(|_| ClaimsSbfError::Accounts)?;
        hash(&bytes).to_bytes()
    };
    let linked_basis_digest = {
        let bytes = base
            .linked_basis_record
            .try_borrow_data()
            .map_err(|_| ClaimsSbfError::Accounts)?;
        hash(&bytes).to_bytes()
    };
    authenticate_runtime_product_basis_core_v2(
        base.registry,
        base.rent,
        base.core_market,
        base.core_program,
        base.linked_basis_record,
        base.linked_basis_staging,
        ProductRuntimeFrameV2 {
            product: FinalizedRecordFrameV2 {
                raw: base.product_record,
                staging: base.product_staging,
            },
            result_domain: FinalizedRecordFrameV2 {
                raw: base.result_domain_record,
                staging: base.result_domain_staging,
            },
            portfolio: FinalizedRecordFrameV2 {
                raw: base.portfolio_record,
                staging: base.portfolio_staging,
            },
        },
        market,
        product_digest,
        linked_basis_digest,
        if header.action == RepresentationActionV2::RedeemTerminal {
            CorePhase::Terminal
        } else {
            CorePhase::Open
        },
    )?;
    let linked_data = base
        .linked_basis_record
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let linked = LinkedBasisRecordV2::decode(&linked_data).map_err(|_| ClaimsSbfError::Identity)?;
    let basis_id = ContentIdV2::new(market.basis_id).map_err(|_| ClaimsSbfError::Identity)?;
    let product_id =
        ContentIdV2::new(market.product_instance_id).map_err(|_| ClaimsSbfError::Identity)?;
    let admitted = AdmittedBasisV2::admit(linked.basis_record(), basis_id, basis_id, product_id)
        .map_err(|_| ClaimsSbfError::Identity)?;
    Ok((market.generation, admitted))
}

fn authenticate_or_allocate_replay<'info>(
    program_id: &Pubkey,
    base: BaseAccounts<'_, 'info>,
    request: RepresentationRequestV2<'_>,
) -> Result<bool, ProgramError> {
    let header = request.header();
    let seeds = [
        RATIONAL_REPLAY_SEED_V2,
        header.descriptor_id.as_slice(),
        header.actor.as_slice(),
    ];
    let (expected, bump) = Pubkey::find_program_address(&seeds, program_id);
    if base.replay.key != &expected {
        return Err(ClaimsSbfError::Identity.into());
    }
    if base.replay.owner == program_id {
        let data = base
            .replay
            .try_borrow_data()
            .map_err(|_| ClaimsSbfError::Accounts)?;
        let replay = RationalReplayV2::decode(&data).map_err(|_| ClaimsSbfError::Representation)?;
        if replay.descriptor() != header.descriptor_id
            || replay.actor() != header.actor
            || replay.revision() != header.expected_representation_revision
        {
            return Err(ClaimsSbfError::Representation.into());
        }
        return Ok(false);
    }
    let rent = Rent::from_account_info(base.rent).map_err(|_| ClaimsSbfError::Accounts)?;
    if header.expected_representation_revision != 0
        || base.replay.owner != &system_program::ID
        || base.replay.data_len() != 0
        || base.replay.lamports() < rent.minimum_balance(RATIONAL_REPLAY_BYTES_V2)
        || base.replay.executable
        || base.replay.is_signer
    {
        return Err(ClaimsSbfError::Accounts.into());
    }
    let bump_seed = [bump];
    let signer = [seeds[0], seeds[1], seeds[2], bump_seed.as_slice()];
    invoke_signed(
        &allocate(
            base.replay.key,
            u64::try_from(RATIONAL_REPLAY_BYTES_V2).map_err(|_| ClaimsSbfError::Accounts)?,
        ),
        &[base.replay.clone(), base.system.clone()],
        &[&signer],
    )
    .map_err(|_| ClaimsSbfError::Accounts)?;
    invoke_signed(
        &assign(base.replay.key, program_id),
        &[base.replay.clone(), base.system.clone()],
        &[&signer],
    )
    .map_err(|_| ClaimsSbfError::Accounts)?;
    if base.replay.owner != program_id || base.replay.data_len() != RATIONAL_REPLAY_BYTES_V2 {
        return Err(ClaimsSbfError::Accounts.into());
    }
    Ok(true)
}

fn build_projection(
    program_id: &Pubkey,
    account_infos: &[AccountInfo<'_>],
    base: BaseAccounts<'_, '_>,
    request: RepresentationRequestV2<'_>,
    descriptor: RepresentationDescriptorV2<'_>,
) -> Result<Vec<u8>, ProgramError> {
    let header = request.header();
    let receipt = parse_mint(base.receipt_mint, base.token_program, header.receipt_mint)?;
    if receipt.supply != header.expected_receipt_supply
        || receipt.mint_authority != COption::Some(header.representation_authority)
        || receipt.decimals != 0
        || receipt.freeze_authority != COption::None
    {
        return Err(ClaimsSbfError::Token.into());
    }
    if matches!(
        header.action,
        RepresentationActionV2::IssueStructured | RepresentationActionV2::UnwrapStructured
    ) {
        let actor_receipt = parse_token_account(
            base.actor_receipt,
            base.token_program,
            header.receipt_mint,
            header.actor,
        )?;
        if header.action == RepresentationActionV2::UnwrapStructured
            && actor_receipt.amount < header.quantity
        {
            return Err(ClaimsSbfError::Token.into());
        }
    }
    let projection_width = usize::try_from(header.outcome_count)
        .ok()
        .and_then(|width| width.checked_mul(STRUCTURED_VECTOR_COUNT))
        .and_then(|width| width.checked_mul(SCALAR_BYTES))
        .and_then(|tail| STRUCTURED_HEADER_BYTES.checked_add(tail))
        .ok_or(ClaimsSbfError::Representation)?;
    let mut projection = vec![0; projection_width];
    StructuredProjectionV2::write_header(
        &mut projection,
        StructuredProjectionHeaderV2 {
            descriptor_id: header.descriptor_id,
            market_id: header.market,
            receipt_mint: header.receipt_mint,
            outcome_count: header.outcome_count,
            denominator: header.denominator,
            receipt_supply: receipt.supply,
            revision: header.expected_representation_revision,
        },
    )
    .map_err(|_| ClaimsSbfError::Representation)?;
    let mut row = 0_u32;
    while row < header.asset_count {
        let outcome = if header.action.selected_outcome() {
            header.selected_outcome
        } else {
            row
        };
        let requested = request
            .asset(row)
            .map_err(|_| ClaimsSbfError::Instruction)?;
        let accounts = asset_accounts(account_infos, row)?;
        let identities = authenticate_asset_identities(
            program_id,
            base,
            header.descriptor_id,
            header.action,
            outcome,
            Some(requested),
            accounts,
        )?;
        let position = accounts
            .position
            .try_borrow_data()
            .map_err(|_| ClaimsSbfError::Accounts)?;
        let position_view =
            PositionViewV2::decode(&position).map_err(|_| ClaimsSbfError::Economic)?;
        let market_data = base
            .aggregate
            .try_borrow_data()
            .map_err(|_| ClaimsSbfError::Accounts)?;
        let market = MarketViewV2::decode(&market_data).map_err(|_| ClaimsSbfError::Economic)?;
        if position_view.market_account != base.aggregate.key.to_bytes()
            || position_view.owner != identities.claims_custody_owner
            || position_view.basis_id != market.basis_id
            || position_view.claim_count != header.outcome_count
        {
            return Err(ClaimsSbfError::Identity.into());
        }
        let native = read_vector(
            &position,
            LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
            header.outcome_count,
        )
        .map_err(|_| ClaimsSbfError::Economic)?;
        let native_locked = *native
            .get(usize::try_from(outcome).map_err(|_| ClaimsSbfError::Economic)?)
            .ok_or(ClaimsSbfError::Economic)?;
        if header.action.uses_claims()
            && outcome == header.selected_outcome
            && position_view.revision != header.expected_custody_position_revision
        {
            return Err(ClaimsSbfError::Identity.into());
        }
        drop(position);
        let mint = parse_mint(accounts.mint, base.token_program, identities.shard_mint)?;
        let actor = parse_token_account(
            accounts.actor_token,
            base.token_program,
            identities.shard_mint,
            header.actor,
        )?;
        let structured = parse_token_account(
            accounts.structured_token,
            base.token_program,
            identities.shard_mint,
            header.representation_authority,
        )?;
        if mint.mint_authority != COption::Some(header.representation_authority)
            || mint.decimals != 0
            || mint.freeze_authority != COption::None
        {
            return Err(ClaimsSbfError::Token.into());
        }
        if requested.expected_shard_supply != mint.supply
            || requested.expected_actor_shards != actor.amount
            || requested.expected_structured_shards != structured.amount
        {
            return Err(ClaimsSbfError::Token.into());
        }
        let explicit_free = mint
            .supply
            .checked_sub(structured.amount)
            .ok_or(ClaimsSbfError::Token)?;
        StructuredProjectionV2::write_coordinate(
            &mut projection,
            header.outcome_count,
            outcome,
            CoordinateObservation {
                coefficient: descriptor
                    .coefficient(outcome)
                    .map_err(|_| ClaimsSbfError::Representation)?,
                native_locked,
                shard_supply: mint.supply,
                structured_custody: structured.amount,
                explicit_free_shards: explicit_free,
            },
        )
        .map_err(|_| ClaimsSbfError::Representation)?;
        row = row.checked_add(1).ok_or(ClaimsSbfError::Representation)?;
    }
    Ok(projection)
}

#[derive(Clone, Copy)]
struct CoordinateIdentities {
    shard_mint: [u8; 32],
    claims_custody_owner: [u8; 32],
}

#[allow(clippy::too_many_arguments)]
fn authenticate_asset_identities(
    program_id: &Pubkey,
    base: BaseAccounts<'_, '_>,
    descriptor: [u8; 32],
    action: RepresentationActionV2,
    outcome: u32,
    requested: Option<dclutch_rational_representation_v2_contract::AssetV2>,
    accounts: AssetAccounts<'_, '_>,
) -> Result<CoordinateIdentities, ProgramError> {
    let outcome_bytes = outcome.to_le_bytes();
    let mint = Pubkey::find_program_address(
        &[RATIONAL_SHARD_MINT_SEED_V2, &descriptor, &outcome_bytes],
        program_id,
    )
    .0;
    let custody_owner = Pubkey::find_program_address(
        &[
            RATIONAL_CLAIMS_CUSTODY_OWNER_SEED_V2,
            &descriptor,
            &outcome_bytes,
        ],
        program_id,
    )
    .0;
    let position_seeds =
        ProtocolPositionSeedsV2::new(base.aggregate.key.to_bytes(), custody_owner.to_bytes())
            .map_err(|_| ClaimsSbfError::Identity)?;
    let expected_position = Pubkey::find_program_address(&position_seeds.as_slices(), program_id).0;
    let structured = get_associated_token_address_with_program_id(
        base.representation_authority.key,
        &mint,
        base.token_program.key,
    );
    if accounts.position.key != &expected_position
        || accounts.position.owner != program_id
        || accounts.mint.key != &mint
        || accounts.mint.owner != base.token_program.key
        || accounts.structured_token.key != &structured
        || accounts.structured_token.owner != base.token_program.key
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    if let Some(requested) = requested {
        if requested.shard_mint != mint.to_bytes()
            || requested.claims_custody_owner != custody_owner.to_bytes()
            || requested.structured_custody_account != structured.to_bytes()
            || accounts.actor_token.key.to_bytes() != requested.actor_shard_account
            || accounts.actor_token.owner != base.token_program.key
        {
            return Err(ClaimsSbfError::Identity.into());
        }
    } else if accounts.actor_token.key != base.claims_program.key
        || accounts.actor_token.is_writable
        || !accounts.actor_token.executable
    {
        return Err(ClaimsSbfError::Accounts.into());
    }
    let claims_writable = action.uses_claims() && requested.is_some();
    let shard_mint_writable = matches!(
        action,
        RepresentationActionV2::Denominate
            | RepresentationActionV2::Reconstitute
            | RepresentationActionV2::RedeemTerminal
    ) && requested.is_some();
    let structured_writable = matches!(
        action,
        RepresentationActionV2::IssueStructured | RepresentationActionV2::UnwrapStructured
    );
    if accounts.position.is_writable != claims_writable
        || accounts.position.is_signer
        || accounts.position.executable
        || accounts.mint.is_writable != shard_mint_writable
        || accounts.mint.is_signer
        || accounts.mint.executable
        || accounts.actor_token.is_writable != requested.is_some()
        || accounts.actor_token.is_signer
        || (requested.is_some() && accounts.actor_token.executable)
        || accounts.structured_token.is_writable != structured_writable
        || accounts.structured_token.is_signer
        || accounts.structured_token.executable
    {
        return Err(ClaimsSbfError::Accounts.into());
    }
    Ok(CoordinateIdentities {
        shard_mint: mint.to_bytes(),
        claims_custody_owner: custody_owner.to_bytes(),
    })
}

fn token_effect_digest(prepared: PreparedRepresentationV2<'_>) -> Result<[u8; 32], ProgramError> {
    let mut transcript = Vec::new();
    for effect in prepared.token_effects() {
        let effect = effect.map_err(|_| ClaimsSbfError::Representation)?;
        transcript.push(token_effect_tag(effect.style));
        transcript.extend_from_slice(&effect.mint);
        transcript.extend_from_slice(&effect.source);
        transcript.extend_from_slice(&effect.destination);
        transcript.extend_from_slice(&effect.authority);
        transcript.extend_from_slice(&effect.amount.to_le_bytes());
    }
    Ok(hashv(&[b"dclutch:rational-token-effects:v2", &transcript]).to_bytes())
}

fn execute_token_effects<'accounts, 'info>(
    program_id: &Pubkey,
    account_infos: &'accounts [AccountInfo<'info>],
    base: BaseAccounts<'accounts, 'info>,
    prepared: PreparedRepresentationV2<'_>,
) -> Result<(), ProgramError> {
    let request = prepared.request();
    let header = request.header();
    let (_, bump) = Pubkey::find_program_address(
        &[
            RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2,
            &header.descriptor_id,
        ],
        program_id,
    );
    let bump_seed = [bump];
    let signer = [
        RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2,
        header.descriptor_id.as_slice(),
        bump_seed.as_slice(),
    ];
    for (cursor, effect) in prepared.token_effects().enumerate() {
        let effect = effect.map_err(|_| ClaimsSbfError::Representation)?;
        match effect.style {
            TokenEffectStyleV2::TransferShardToStructured => {
                let row = u32::try_from(cursor).map_err(|_| ClaimsSbfError::Token)?;
                let accounts = asset_accounts(account_infos, row)?;
                require_effect_accounts(
                    effect,
                    accounts.mint,
                    accounts.actor_token,
                    accounts.structured_token,
                    base.actor,
                )?;
                let instruction = token_instruction::transfer_checked(
                    base.token_program.key,
                    accounts.actor_token.key,
                    accounts.mint.key,
                    accounts.structured_token.key,
                    base.actor.key,
                    &[],
                    effect.amount,
                    0,
                )
                .map_err(|_| ClaimsSbfError::Token)?;
                invoke(
                    &instruction,
                    &[
                        accounts.actor_token.clone(),
                        accounts.mint.clone(),
                        accounts.structured_token.clone(),
                        base.actor.clone(),
                        base.token_program.clone(),
                    ],
                )
                .map_err(|_| ClaimsSbfError::Token)?;
            }
            TokenEffectStyleV2::TransferShardFromStructured => {
                let row = cursor.checked_sub(1).ok_or(ClaimsSbfError::Token)?;
                let row = u32::try_from(row).map_err(|_| ClaimsSbfError::Token)?;
                let accounts = asset_accounts(account_infos, row)?;
                require_effect_accounts(
                    effect,
                    accounts.mint,
                    accounts.structured_token,
                    accounts.actor_token,
                    base.representation_authority,
                )?;
                let instruction = token_instruction::transfer_checked(
                    base.token_program.key,
                    accounts.structured_token.key,
                    accounts.mint.key,
                    accounts.actor_token.key,
                    base.representation_authority.key,
                    &[],
                    effect.amount,
                    0,
                )
                .map_err(|_| ClaimsSbfError::Token)?;
                invoke_signed(
                    &instruction,
                    &[
                        accounts.structured_token.clone(),
                        accounts.mint.clone(),
                        accounts.actor_token.clone(),
                        base.representation_authority.clone(),
                        base.token_program.clone(),
                    ],
                    &[&signer],
                )
                .map_err(|_| ClaimsSbfError::Token)?;
            }
            TokenEffectStyleV2::MintReceipt => {
                require_effect_accounts(
                    effect,
                    base.receipt_mint,
                    base.claims_program,
                    base.actor_receipt,
                    base.representation_authority,
                )?;
                let instruction = token_instruction::mint_to_checked(
                    base.token_program.key,
                    base.receipt_mint.key,
                    base.actor_receipt.key,
                    base.representation_authority.key,
                    &[],
                    effect.amount,
                    0,
                )
                .map_err(|_| ClaimsSbfError::Token)?;
                invoke_signed(
                    &instruction,
                    &[
                        base.receipt_mint.clone(),
                        base.actor_receipt.clone(),
                        base.representation_authority.clone(),
                        base.token_program.clone(),
                    ],
                    &[&signer],
                )
                .map_err(|_| ClaimsSbfError::Token)?;
            }
            TokenEffectStyleV2::BurnReceipt => {
                require_effect_accounts(
                    effect,
                    base.receipt_mint,
                    base.actor_receipt,
                    base.claims_program,
                    base.actor,
                )?;
                burn(
                    base,
                    base.actor_receipt,
                    base.receipt_mint,
                    base.actor,
                    effect.amount,
                )?;
            }
            TokenEffectStyleV2::BurnShard => {
                let accounts = asset_accounts(account_infos, 0)?;
                require_effect_accounts(
                    effect,
                    accounts.mint,
                    accounts.actor_token,
                    base.claims_program,
                    base.actor,
                )?;
                burn(
                    base,
                    accounts.actor_token,
                    accounts.mint,
                    base.actor,
                    effect.amount,
                )?;
            }
            TokenEffectStyleV2::MintShard => {
                let accounts = asset_accounts(account_infos, 0)?;
                require_effect_accounts(
                    effect,
                    accounts.mint,
                    base.claims_program,
                    accounts.actor_token,
                    base.representation_authority,
                )?;
                let instruction = token_instruction::mint_to_checked(
                    base.token_program.key,
                    accounts.mint.key,
                    accounts.actor_token.key,
                    base.representation_authority.key,
                    &[],
                    effect.amount,
                    0,
                )
                .map_err(|_| ClaimsSbfError::Token)?;
                invoke_signed(
                    &instruction,
                    &[
                        accounts.mint.clone(),
                        accounts.actor_token.clone(),
                        base.representation_authority.clone(),
                        base.token_program.clone(),
                    ],
                    &[&signer],
                )
                .map_err(|_| ClaimsSbfError::Token)?;
            }
        }
    }
    Ok(())
}

fn burn<'accounts, 'info>(
    base: BaseAccounts<'accounts, 'info>,
    source: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    amount: u64,
) -> Result<(), ProgramError> {
    let instruction = token_instruction::burn_checked(
        base.token_program.key,
        source.key,
        mint.key,
        authority.key,
        &[],
        amount,
        0,
    )
    .map_err(|_| ClaimsSbfError::Token)?;
    invoke(
        &instruction,
        &[
            source.clone(),
            mint.clone(),
            authority.clone(),
            base.token_program.clone(),
        ],
    )
    .map_err(|_| ClaimsSbfError::Token.into())
}

fn execute_claims_if_any<'accounts, 'info>(
    program_id: &Pubkey,
    account_infos: &'accounts [AccountInfo<'info>],
    base: BaseAccounts<'accounts, 'info>,
    prepared: PreparedRepresentationV2<'_>,
    request_digest: [u8; 32],
    admitted_basis: AdmittedBasisV2,
) -> Result<Box<ClaimsEvidence>, ProgramError> {
    let request = prepared.request();
    if !request.header().action.uses_claims() {
        return Ok(Box::new(ClaimsEvidence {
            plan_digest: [0; 32],
            packet: Vec::new(),
            context: None,
            receipt: None,
            terminal_request: None,
            terminal_request_digest: [0; 32],
            terminal_receipt: None,
            custody: None,
        }));
    }
    if request.header().action == RepresentationActionV2::RedeemTerminal {
        return execute_terminal_claims(
            program_id,
            account_infos,
            base,
            request,
            request_digest,
            admitted_basis,
        );
    }
    let header = request.header();
    let custody_position = asset_accounts(account_infos, 0)?.position;
    let market_data = base
        .aggregate
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let market = MarketViewV2::decode(&market_data).map_err(|_| ClaimsSbfError::Economic)?;
    drop(market_data);
    let product_record_digest = {
        let data = base
            .product_record
            .try_borrow_data()
            .map_err(|_| ClaimsSbfError::Accounts)?;
        hash(&data).to_bytes()
    };
    let linked_basis_record_digest = {
        let data = base
            .linked_basis_record
            .try_borrow_data()
            .map_err(|_| ClaimsSbfError::Accounts)?;
        hash(&data).to_bytes()
    };
    let context = AffineBatchContextV2 {
        product_record_digest,
        semantic_basis_id: market.basis_id,
        linked_basis_record_digest,
    };
    let mut affine_plan = vec![
        0_u8;
        prepared
            .affine_packet_bytes()
            .map_err(|_| ClaimsSbfError::Representation)?
    ];
    let affine_caller_role = prepared
        .write_affine_packet(request_digest, Some(context), &mut affine_plan)
        .map_err(|_| ClaimsSbfError::Representation)?
        .ok_or(ClaimsSbfError::Representation)?
        .caller_role();
    let plan_digest = hash(&affine_plan).to_bytes();
    let affine_accounts = vec![
        base.caller_authority.clone(),
        base.aggregate.clone(),
        base.linked_basis_record.clone(),
        base.linked_basis_staging.clone(),
        base.product_record.clone(),
        base.product_staging.clone(),
        base.result_domain_record.clone(),
        base.result_domain_staging.clone(),
        base.portfolio_record.clone(),
        base.portfolio_staging.clone(),
        base.rent.clone(),
        base.core_market.clone(),
        base.cache.clone(),
        base.registry.clone(),
        base.caller_program.clone(),
        base.caller_programdata.clone(),
        base.claims_program.clone(),
        base.claims_programdata.clone(),
        base.core_program.clone(),
        base.core_programdata.clone(),
        base.actor_position.clone(),
        custody_position.clone(),
    ];
    if affine_accounts.len() != AFFINE_BATCH_FIXED_ACCOUNT_COUNT_V2 + 2 {
        return Err(ClaimsSbfError::Accounts.into());
    }
    let receipt = execute_parent_authenticated(
        program_id,
        &affine_accounts,
        &affine_plan,
        AuthenticatedAffineParentV2 {
            caller_role: affine_caller_role,
            release_set: header.release_set,
            market: header.market,
            parent_context: header.parent_context,
            parent_request_digest: request_digest,
        },
    )?;
    Ok(Box::new(ClaimsEvidence {
        plan_digest,
        packet: affine_plan,
        context: Some(context),
        receipt: Some(receipt),
        terminal_request: None,
        terminal_request_digest: [0; 32],
        terminal_receipt: None,
        custody: None,
    }))
}

#[inline(never)]
fn execute_terminal_claims<'accounts, 'info>(
    program_id: &Pubkey,
    account_infos: &'accounts [AccountInfo<'info>],
    base: BaseAccounts<'accounts, 'info>,
    request: RepresentationRequestV2<'_>,
    request_digest: [u8; 32],
    admitted_basis: AdmittedBasisV2,
) -> Result<Box<ClaimsEvidence>, ProgramError> {
    let header = request.header();
    let offset = terminal_offset(header.asset_count)?;
    if account_infos.len()
        != offset
            .checked_add(RATIONAL_TERMINAL_ACCOUNT_COUNT_V2)
            .ok_or(ClaimsSbfError::Accounts)?
    {
        return Err(ClaimsSbfError::Accounts.into());
    }
    let terminal = terminal_accounts(account_infos, offset)?;
    authenticate_terminal_privileges(terminal)?;
    let custody_position = asset_accounts(account_infos, 0)?.position;
    let terminal_accounts = vec![
        base.actor.clone(),
        base.aggregate.clone(),
        custody_position.clone(),
        base.linked_basis_record.clone(),
        base.linked_basis_staging.clone(),
        base.product_record.clone(),
        base.product_staging.clone(),
        base.rent.clone(),
        base.core_market.clone(),
        terminal.coordinate.clone(),
        terminal.coordinate_staging.clone(),
        base.cache.clone(),
        base.registry.clone(),
        base.claims_program.clone(),
        base.claims_programdata.clone(),
        terminal.custody_program.clone(),
        terminal.custody_programdata.clone(),
        base.core_program.clone(),
        base.core_programdata.clone(),
        terminal.caller_authority.clone(),
        terminal.realm.clone(),
        terminal.realm_staging.clone(),
        terminal.replay.clone(),
        terminal.collateral_mint.clone(),
        terminal.hoard.clone(),
        terminal.recipient.clone(),
        terminal.custody_authority.clone(),
        terminal.token_program.clone(),
    ];
    if terminal_accounts.len() != LIABILITY_BASIS_ACCOUNT_COUNT_V2 {
        return Err(ClaimsSbfError::Accounts.into());
    }
    let executed = execute_parent_authenticated_terminal_v2(
        program_id,
        &terminal_accounts,
        Box::new(AuthenticatedLbv2TerminalParentV2 {
            release_set: header.release_set,
            market: header.market,
            descriptor_id: header.descriptor_id,
            outcome: header.selected_outcome,
            beneficiary_actor: header.actor,
            parent_context: header.parent_context,
            parent_request_digest: request_digest,
            custody_request_nonce: header.expected_representation_revision,
            expected_market_revision: header.expected_claims_market_revision,
            expected_position_revision: header.expected_custody_position_revision,
            expected_custody_revision: header.expected_custody_replay_revision,
            debit_quantity: header.quantity,
            admitted_basis,
        }),
    )?;
    let terminal_request = executed.receipt.request();
    let terminal_request_digest = executed.receipt.request_digest();
    let custody = executed.custody.map(|evidence| {
        Box::new(CustodyEvidence {
            request: evidence.request,
            request_digest: evidence.request_digest,
            receipt: evidence.receipt,
            receipt_digest: evidence.receipt_digest,
            replay_digest: evidence.replay_digest,
        })
    });
    Ok(Box::new(ClaimsEvidence {
        plan_digest: [0; 32],
        packet: Vec::new(),
        context: None,
        receipt: None,
        terminal_request: Some(Box::new(terminal_request)),
        terminal_request_digest,
        terminal_receipt: Some(executed.receipt),
        custody,
    }))
}

fn terminal_offset(asset_count: u32) -> Result<usize, ProgramError> {
    usize::try_from(asset_count)
        .ok()
        .and_then(|count| count.checked_mul(RATIONAL_ASSET_ACCOUNT_COUNT_V2))
        .and_then(|count| RATIONAL_BASE_ACCOUNT_COUNT_V2.checked_add(count))
        .ok_or_else(|| ClaimsSbfError::Accounts.into())
}

fn terminal_accounts<'accounts, 'info>(
    account_infos: &'accounts [AccountInfo<'info>],
    offset: usize,
) -> Result<TerminalAccounts<'accounts, 'info>, ProgramError> {
    Ok(TerminalAccounts {
        caller_authority: account(account_infos, offset + TERMINAL_CALLER_AUTHORITY)?,
        custody_program: account(account_infos, offset + TERMINAL_CUSTODY_PROGRAM)?,
        custody_programdata: account(account_infos, offset + TERMINAL_CUSTODY_PROGRAMDATA)?,
        coordinate: account(account_infos, offset + TERMINAL_COORDINATE)?,
        coordinate_staging: account(account_infos, offset + TERMINAL_COORDINATE_STAGING)?,
        realm: account(account_infos, offset + TERMINAL_REALM)?,
        realm_staging: account(account_infos, offset + TERMINAL_REALM_STAGING)?,
        replay: account(account_infos, offset + TERMINAL_CUSTODY_REPLAY)?,
        collateral_mint: account(account_infos, offset + TERMINAL_COLLATERAL_MINT)?,
        hoard: account(account_infos, offset + TERMINAL_HOARD)?,
        recipient: account(account_infos, offset + TERMINAL_RECIPIENT)?,
        custody_authority: account(account_infos, offset + TERMINAL_CUSTODY_AUTHORITY)?,
        token_program: account(account_infos, offset + TERMINAL_TOKEN_PROGRAM)?,
    })
}

fn authenticate_terminal_privileges(
    terminal: TerminalAccounts<'_, '_>,
) -> Result<(), ProgramError> {
    if terminal.caller_authority.is_signer
        || terminal.caller_authority.is_writable
        || terminal.caller_authority.executable
        || !terminal.custody_program.executable
        || terminal.custody_program.is_signer
        || terminal.custody_program.is_writable
        || terminal.custody_programdata.executable
        || terminal.custody_programdata.is_signer
        || terminal.custody_programdata.is_writable
        || terminal.coordinate.executable
        || terminal.coordinate.is_signer
        || terminal.coordinate.is_writable
        || terminal.coordinate_staging.executable
        || terminal.coordinate_staging.is_signer
        || terminal.coordinate_staging.is_writable
        || terminal.realm.executable
        || terminal.realm.is_signer
        || terminal.realm.is_writable
        || terminal.realm_staging.executable
        || terminal.realm_staging.is_signer
        || terminal.realm_staging.is_writable
        || terminal.realm_staging.owner != &system_program::ID
        || terminal.realm_staging.data_len() != 0
        || terminal.replay.executable
        || terminal.replay.is_signer
        || !terminal.replay.is_writable
        || terminal.collateral_mint.executable
        || terminal.collateral_mint.is_signer
        || terminal.collateral_mint.is_writable
        || terminal.hoard.executable
        || terminal.hoard.is_signer
        || !terminal.hoard.is_writable
        || terminal.recipient.executable
        || terminal.recipient.is_signer
        || !terminal.recipient.is_writable
        || terminal.custody_authority.executable
        || terminal.custody_authority.is_signer
        || terminal.custody_authority.is_writable
        || !terminal.token_program.executable
        || terminal.token_program.is_signer
        || terminal.token_program.is_writable
    {
        return Err(ClaimsSbfError::Accounts.into());
    }
    Ok(())
}

fn post_token_observations(
    account_infos: &[AccountInfo<'_>],
    base: BaseAccounts<'_, '_>,
    request: RepresentationRequestV2<'_>,
) -> Result<(Vec<u8>, u64), ProgramError> {
    let header = request.header();
    let receipt = parse_mint(base.receipt_mint, base.token_program, header.receipt_mint)?;
    let capacity = usize::try_from(header.asset_count)
        .map_err(|_| ClaimsSbfError::Token)?
        .checked_mul(24)
        .ok_or(ClaimsSbfError::Token)?;
    let mut output = Vec::with_capacity(capacity);
    let mut row = 0_u32;
    while row < header.asset_count {
        let requested = request
            .asset(row)
            .map_err(|_| ClaimsSbfError::Instruction)?;
        let accounts = asset_accounts(account_infos, row)?;
        let mint = parse_mint(accounts.mint, base.token_program, requested.shard_mint)?;
        let actor = parse_token_account(
            accounts.actor_token,
            base.token_program,
            requested.shard_mint,
            header.actor,
        )?;
        let structured = parse_token_account(
            accounts.structured_token,
            base.token_program,
            requested.shard_mint,
            header.representation_authority,
        )?;
        output.extend_from_slice(&mint.supply.to_le_bytes());
        output.extend_from_slice(&actor.amount.to_le_bytes());
        output.extend_from_slice(&structured.amount.to_le_bytes());
        row = row.checked_add(1).ok_or(ClaimsSbfError::Token)?;
    }
    Ok((output, receipt.supply))
}

fn post_resource_digest(
    account_infos: &[AccountInfo<'_>],
    base: BaseAccounts<'_, '_>,
    request: RepresentationRequestV2<'_>,
    custody: Option<&CustodyEvidence>,
) -> Result<[u8; 32], ProgramError> {
    let header = request.header();
    let mut digests = Vec::new();
    let next_replay = RationalReplayV2::new(
        header.descriptor_id,
        header.actor,
        header
            .expected_representation_revision
            .checked_add(1)
            .ok_or(ClaimsSbfError::Representation)?,
    )
    .map_err(|_| ClaimsSbfError::Representation)?
    .to_bytes();
    digests.extend_from_slice(&hash(&next_replay).to_bytes());
    for fixed in [base.aggregate, base.receipt_mint] {
        let data = fixed
            .try_borrow_data()
            .map_err(|_| ClaimsSbfError::Accounts)?;
        digests.extend_from_slice(&hash(&data).to_bytes());
    }
    let mut row = 0_u32;
    while row < header.asset_count {
        let accounts = asset_accounts(account_infos, row)?;
        for dynamic in [accounts.position, accounts.mint, accounts.structured_token] {
            let data = dynamic
                .try_borrow_data()
                .map_err(|_| ClaimsSbfError::Accounts)?;
            digests.extend_from_slice(&hash(&data).to_bytes());
        }
        let data = accounts
            .actor_token
            .try_borrow_data()
            .map_err(|_| ClaimsSbfError::Accounts)?;
        digests.extend_from_slice(&hash(&data).to_bytes());
        row = row.checked_add(1).ok_or(ClaimsSbfError::Receipt)?;
    }
    if let Some(custody) = custody {
        digests.extend_from_slice(&custody.receipt_digest);
        digests.extend_from_slice(&custody.replay_digest);
    }
    Ok(hashv(&[b"dclutch:rational-post-resources:v2", &digests]).to_bytes())
}

fn commit_replay(
    base: BaseAccounts<'_, '_>,
    request: RepresentationRequestV2<'_>,
    fresh: bool,
) -> Result<(), ProgramError> {
    let header = request.header();
    let next = header
        .expected_representation_revision
        .checked_add(1)
        .ok_or(ClaimsSbfError::Representation)?;
    let encoded = RationalReplayV2::new(header.descriptor_id, header.actor, next)
        .map_err(|_| ClaimsSbfError::Representation)?
        .to_bytes();
    let mut data = base
        .replay
        .try_borrow_mut_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    if data.len() != encoded.len()
        || (!fresh
            && RationalReplayV2::decode(&data)
                .map_err(|_| ClaimsSbfError::Representation)?
                .revision()
                != header.expected_representation_revision)
    {
        return Err(ClaimsSbfError::Representation.into());
    }
    data.copy_from_slice(&encoded);
    Ok(())
}

fn parse_mint(
    account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    expected: [u8; 32],
) -> Result<Mint, ProgramError> {
    if account.key.to_bytes() != expected || account.owner != token_program.key {
        return Err(ClaimsSbfError::Token.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    if data.len() != MINT_BYTES {
        return Err(ClaimsSbfError::Token.into());
    }
    let mint = Mint::parse(&data).map_err(|_| ClaimsSbfError::Token)?;
    if !mint.is_initialized {
        return Err(ClaimsSbfError::Token.into());
    }
    Ok(mint)
}

fn parse_token_account(
    account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    mint: [u8; 32],
    owner: [u8; 32],
) -> Result<TokenAccount, ProgramError> {
    if account.owner != token_program.key {
        return Err(ClaimsSbfError::Token.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    if data.len() != ACCOUNT_BYTES {
        return Err(ClaimsSbfError::Token.into());
    }
    let token = TokenAccount::parse(&data).map_err(|_| ClaimsSbfError::Token)?;
    if token.mint != mint
        || token.owner != owner
        || token.state != AccountState::Initialized
        || token.native_reserve != COption::None
        || token.delegate != COption::None
        || token.delegated_amount != 0
        || token.close_authority != COption::None
    {
        return Err(ClaimsSbfError::Token.into());
    }
    Ok(token)
}

fn require_effect_accounts(
    effect: dclutch_rational_representation_v2_contract::TokenEffectV2,
    mint: &AccountInfo<'_>,
    source: &AccountInfo<'_>,
    destination: &AccountInfo<'_>,
    authority: &AccountInfo<'_>,
) -> Result<(), ProgramError> {
    if effect.mint != mint.key.to_bytes()
        || (effect.source != [0; 32] && effect.source != source.key.to_bytes())
        || (effect.destination != [0; 32] && effect.destination != destination.key.to_bytes())
        || effect.authority != authority.key.to_bytes()
    {
        return Err(ClaimsSbfError::Token.into());
    }
    Ok(())
}

fn asset_accounts<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    row: u32,
) -> Result<AssetAccounts<'accounts, 'info>, ProgramError> {
    let row = usize::try_from(row).map_err(|_| ClaimsSbfError::Accounts)?;
    let base = row
        .checked_mul(RATIONAL_ASSET_ACCOUNT_COUNT_V2)
        .and_then(|value| RATIONAL_BASE_ACCOUNT_COUNT_V2.checked_add(value))
        .ok_or(ClaimsSbfError::Accounts)?;
    Ok(AssetAccounts {
        position: account(accounts, base + ASSET_POSITION)?,
        mint: account(accounts, base + ASSET_SHARD_MINT)?,
        actor_token: account(accounts, base + ASSET_ACTOR_TOKEN)?,
        structured_token: account(accounts, base + ASSET_STRUCTURED_TOKEN)?,
    })
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| ClaimsSbfError::Accounts.into())
}

fn caller_role(role: dclutch_rational_representation_v2_contract::CallerRoleV2) -> ExecutionRoleV1 {
    match role {
        dclutch_rational_representation_v2_contract::CallerRoleV2::Core => ExecutionRoleV1::Core,
        dclutch_rational_representation_v2_contract::CallerRoleV2::Trading => {
            ExecutionRoleV1::Trading
        }
    }
}

const fn token_effect_tag(style: TokenEffectStyleV2) -> u8 {
    match style {
        TokenEffectStyleV2::MintShard => 1,
        TokenEffectStyleV2::BurnShard => 2,
        TokenEffectStyleV2::TransferShardToStructured => 3,
        TokenEffectStyleV2::TransferShardFromStructured => 4,
        TokenEffectStyleV2::MintReceipt => 5,
        TokenEffectStyleV2::BurnReceipt => 6,
    }
}

fn request_digest_of(bytes: &[u8]) -> [u8; 32] {
    hash(bytes).to_bytes()
}
