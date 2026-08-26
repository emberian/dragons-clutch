//! Claims-owned physical adapter for exact rational representations.
//!
//! Immutable finalized descriptor and graph records own interpretation. Token
//! programs own Mint supplies and holder balances. The canonical Claims
//! economic kernel owns native and materialized quantities. This module owns
//! only one per-descriptor/actor replay revision and commits it after every
//! Claims, Token, and Custody postcondition has passed.

use dclutch_claims_svm::{
    affine_batch_v2::{AffineBatchPlanV2, AffineBatchReceiptV2},
    protocol_position_v2::{ProtocolPositionClaimsCapabilitySeedsV2, ProtocolPositionSeedsV2},
    signed_delta_v3::{SignedDeltaPlanV3, SignedDeltaReceiptV3},
};
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{CustodyReceiptV1, CustodyRequestV1};
use dclutch_rational_representation_v2_contract::{
    AffineBatchContextV2, CompletionEvidenceV2, PreparedRepresentationV2,
    RATIONAL_ASSET_ACCOUNT_COUNT_V2, RATIONAL_BASE_ACCOUNT_COUNT_V2,
    RATIONAL_TERMINAL_ACCOUNT_COUNT_V2, RationalReplayV2, RepresentationActionV2,
    RepresentationRequestV2, TokenEffectStyleV2, finalize, prepare,
};
use dclutch_rational_representation_v2_kernel::{
    CoordinateObservation, DescriptorAdmissionV2, RepresentationDescriptorV2, SCALAR_BYTES,
    STRUCTURED_HEADER_BYTES, STRUCTURED_VECTOR_COUNT, StructuredProjectionHeaderV2,
    StructuredProjectionV2,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_svm::batch_v2::{AuthenticatedRoleBatchReceiptV2, RoleBatchRequestV2};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_representation_composition_v3_kernel::{
    CompositionExposureBundleV3, RecordAdmissionV3,
};
use dclutch_token_svm::{
    TOKEN_2022_PROGRAM_ID, Token2022BehaviorAccountFactsV2, Token2022BehaviorMintFactsV2,
    Token2022BehaviorProfileV2,
};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke, invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::{allocate, assign};
use spl_token_2022_interface::extension::permissioned_burn::instruction as permissioned_burn_instruction;
use spl_token_2022_interface::instruction as token_instruction;

use super::ClaimsSbfError;
use crate::{
    affine_batch_v2::{
        AFFINE_BATCH_FIXED_ACCOUNT_COUNT_V2, AuthenticatedAffineParentV2,
        execute_parent_authenticated,
    },
    liability_basis_v2::{
        LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, MarketViewV2, PositionViewV2, read_vector,
    },
    rational_product_v3::{
        AuthenticatedRationalProductV3, RationalProductFrameV3, authenticate_rational_product_v3,
    },
    rational_terminal_v3::{RationalTerminalFrameV3, execute_rational_terminal_v3},
};

pub use dclutch_rational_representation_v2_contract::{
    RATIONAL_REPLAY_BYTES_V2, RATIONAL_REPLAY_MAGIC_V2, RATIONAL_REPLAY_SEED_V2,
    RATIONAL_REPLAY_VERSION_V2, RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2,
    RATIONAL_SHARD_MINT_SEED_V2, RATIONAL_STRUCTURED_CUSTODY_SEED_V2,
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

const fn rational_product_frame<'accounts, 'info>(
    base: BaseAccounts<'accounts, 'info>,
) -> RationalProductFrameV3<'accounts, 'info> {
    RationalProductFrameV3 {
        aggregate: base.aggregate,
        actor_position: base.actor_position,
        linked_basis_record: base.linked_basis_record,
        linked_basis_staging: base.linked_basis_staging,
        product_record: base.product_record,
        product_staging: base.product_staging,
        result_domain_record: base.result_domain_record,
        result_domain_staging: base.result_domain_staging,
        portfolio_record: base.portfolio_record,
        portfolio_staging: base.portfolio_staging,
        descriptor_record: base.descriptor_raw,
        descriptor_staging: base.descriptor_staging,
        graph_record: base.graph_raw,
        graph_staging: base.graph_staging,
        receipt_mint: base.receipt_mint,
        token_program: base.token_program,
        rent: base.rent,
        registry: base.registry,
        core_market: base.core_market,
        core_program: base.core_program,
        claims_program: base.claims_program,
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
    signed_delta_receipt: Option<SignedDeltaReceiptV3>,
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
    authenticate_execution_releases(base, request)?;
    let authenticated =
        authenticate_rational_product_v3(program_id, rational_product_frame(base), request)?;
    let descriptor_data = base
        .descriptor_raw
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let descriptor = RepresentationDescriptorV2::decode(
        &descriptor_data,
        DescriptorAdmissionV2 {
            selected_descriptor_id: header.descriptor_id,
            finalized_descriptor_id: header.descriptor_id,
            recomputed_descriptor_digest: header.descriptor_id,
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
    let exposure = CompositionExposureBundleV3::decode(
        &graph_data,
        RecordAdmissionV3 {
            selected_id: header.graph_id,
            finalized_id: header.graph_id,
            recomputed_digest: authenticated.admission.graph_digest(),
            finalized_digest: authenticated.admission.graph_digest(),
            record_authenticated: true,
        },
    )
    .map_err(|_| ClaimsSbfError::Representation)?;

    let replay_fresh = authenticate_or_allocate_replay(program_id, base, request)?;
    let projection_bytes = build_projection(program_id, account_infos, base, request, descriptor)?;
    let projection = StructuredProjectionV2::decode(&projection_bytes)
        .map_err(|_| ClaimsSbfError::Representation)?;
    let prepared = prepare(request, descriptor, projection, exposure)
        .map_err(|_| ClaimsSbfError::Representation)?;

    execute_prepared(
        program_id,
        account_infos,
        base,
        prepared,
        request_digest,
        authenticated,
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
    authenticated: Box<AuthenticatedRationalProductV3>,
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
        authenticated,
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
        post_token_observations(account_infos, base, prepared)?;
    let post_resource_digest = post_resource_digest(account_infos, base, request, custody)?;
    let affine_packet = if matches!(
        request.header().action,
        RepresentationActionV2::Denominate | RepresentationActionV2::Reconstitute
    ) {
        Some(AffineBatchPlanV2::decode(&claims.packet).map_err(|_| ClaimsSbfError::Receipt)?)
    } else {
        None
    };
    let signed_delta_packet = if request.header().action == RepresentationActionV2::RedeemTerminal {
        Some(SignedDeltaPlanV3::decode(&claims.packet).map_err(|_| ClaimsSbfError::Receipt)?)
    } else {
        None
    };
    let evidence = CompletionEvidenceV2 {
        request_digest,
        representation_program: program_id.to_bytes(),
        claims_program: program_id.to_bytes(),
        claims_packet_digest: claims.plan_digest,
        affine_packet,
        affine_context: claims.context,
        affine_receipt: claims.receipt,
        signed_delta_packet,
        signed_delta_receipt: claims.signed_delta_receipt,
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
        || header.token_program != TOKEN_2022_PROGRAM_ID
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

fn authenticate_execution_releases(
    base: BaseAccounts<'_, '_>,
    request: RepresentationRequestV2<'_>,
) -> Result<(), ProgramError> {
    let header = request.header();
    let mut entries = Vec::from([
        (
            ExecutionRoleV1::Core,
            base.core_program,
            base.core_programdata,
        ),
        (
            ExecutionRoleV1::Claims,
            base.claims_program,
            base.claims_programdata,
        ),
    ]);
    match caller_role(header.caller_role) {
        ExecutionRoleV1::Core => {
            if base.caller_program.key != base.core_program.key
                || base.caller_programdata.key != base.core_programdata.key
            {
                return Err(ClaimsSbfError::Release.into());
            }
        }
        ExecutionRoleV1::Trading => entries.push((
            ExecutionRoleV1::Trading,
            base.caller_program,
            base.caller_programdata,
        )),
        _ => return Err(ClaimsSbfError::Release.into()),
    }
    authenticate_release_batch(base.registry, base.cache, header.release_set, &entries)
}

#[inline(never)]
fn authenticate_release_batch<'info>(
    registry: &AccountInfo<'info>,
    cache: &AccountInfo<'info>,
    release_set: [u8; 32],
    entries: &[(ExecutionRoleV1, &AccountInfo<'info>, &AccountInfo<'info>)],
) -> Result<(), ProgramError> {
    let cache_digest = {
        let bytes = cache
            .try_borrow_data()
            .map_err(|_| ClaimsSbfError::Release)?;
        ContentId::new(hash(&bytes).to_bytes()).map_err(|_| ClaimsSbfError::Release)?
    };
    let roles = entries.iter().map(|entry| entry.0).collect::<Vec<_>>();
    let request = RoleBatchRequestV2::new(
        ContentId::new(release_set).map_err(|_| ClaimsSbfError::Release)?,
        cache_digest,
        &roles,
    )
    .map_err(|_| ClaimsSbfError::Release)?;
    let request_bytes = request.to_bytes();
    let mut metas = Vec::with_capacity(1 + entries.len() * 2);
    let mut infos = Vec::with_capacity(2 + entries.len() * 2);
    metas.push(AccountMeta::new_readonly(*cache.key, false));
    infos.push(cache.clone());
    for (_, program, programdata) in entries {
        metas.push(AccountMeta::new_readonly(*program.key, false));
        metas.push(AccountMeta::new_readonly(*programdata.key, false));
        infos.push((*program).clone());
        infos.push((*programdata).clone());
    }
    infos.push(registry.clone());
    invoke(
        &Instruction {
            program_id: *registry.key,
            accounts: metas,
            data: request_bytes.to_vec(),
        },
        &infos,
    )
    .map_err(|_| ClaimsSbfError::Release)?;
    let (producer, receipt_bytes) = get_return_data().ok_or(ClaimsSbfError::Release)?;
    if producer != *registry.key {
        return Err(ClaimsSbfError::Release.into());
    }
    let receipt = AuthenticatedRoleBatchReceiptV2::decode(&receipt_bytes)
        .map_err(|_| ClaimsSbfError::Release)?;
    let request_digest =
        ContentId::new(hash(&request_bytes).to_bytes()).map_err(|_| ClaimsSbfError::Release)?;
    if receipt.registry_program().to_bytes() != registry.key.to_bytes()
        || receipt.activation_cache() != cache.key.to_bytes()
        || receipt.activation_cache_digest() != cache_digest
        || receipt.release_set_id().to_bytes() != release_set
        || receipt.request_digest() != request_digest
        || receipt.role_count() != request.role_count()
        || receipt.role_mask() != request.role_mask()
    {
        return Err(ClaimsSbfError::Release.into());
    }
    for (index, (role, program, programdata)) in entries.iter().copied().enumerate() {
        let observation = receipt
            .observation(index)
            .ok_or(ClaimsSbfError::Release)?
            .map_err(|_| ClaimsSbfError::Release)?;
        if observation.role() != role
            || observation.program().to_bytes() != program.key.to_bytes()
            || observation.programdata() != programdata.key.to_bytes()
        {
            return Err(ClaimsSbfError::Release.into());
        }
    }
    Ok(())
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
    let receipt = parse_behavior_mint(
        base.receipt_mint,
        base.token_program,
        header.receipt_mint,
        header.representation_authority,
        header.expected_receipt_supply,
    )?
    .mint();
    if matches!(
        header.action,
        RepresentationActionV2::IssueStructured | RepresentationActionV2::UnwrapStructured
    ) {
        parse_behavior_token_account(
            base.actor_receipt,
            base.token_program,
            header.receipt_mint,
            header.actor,
            if header.action == RepresentationActionV2::UnwrapStructured {
                header.quantity
            } else {
                0
            },
        )?;
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
        let mint = parse_behavior_mint(
            accounts.mint,
            base.token_program,
            identities.shard_mint,
            header.representation_authority,
            requested.expected_shard_supply,
        )?
        .mint();
        let actor = parse_behavior_token_account(
            accounts.actor_token,
            base.token_program,
            identities.shard_mint,
            header.actor,
            requested.expected_actor_shards,
        )?
        .account();
        let structured = parse_behavior_token_account(
            accounts.structured_token,
            base.token_program,
            identities.shard_mint,
            header.representation_authority,
            requested.expected_structured_shards,
        )?
        .account();
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
    let custody_owner_seeds = ProtocolPositionClaimsCapabilitySeedsV2::new(descriptor, outcome)
        .map_err(|_| ClaimsSbfError::Identity)?;
    let custody_owner =
        Pubkey::find_program_address(&custody_owner_seeds.as_slices(), program_id).0;
    let position_seeds =
        ProtocolPositionSeedsV2::new(base.aggregate.key.to_bytes(), custody_owner.to_bytes())
            .map_err(|_| ClaimsSbfError::Identity)?;
    let expected_position = Pubkey::find_program_address(&position_seeds.as_slices(), program_id).0;
    let structured = Pubkey::find_program_address(
        &[
            RATIONAL_STRUCTURED_CUSTODY_SEED_V2,
            &descriptor,
            &outcome_bytes,
        ],
        program_id,
    )
    .0;
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

#[inline(never)]
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

fn pre_effect_mint_facts(
    base: BaseAccounts<'_, '_>,
    request: RepresentationRequestV2<'_>,
    mint: &AccountInfo<'_>,
) -> Result<Token2022BehaviorMintFactsV2, ProgramError> {
    parse_behavior_mint(
        mint,
        base.token_program,
        mint.key.to_bytes(),
        request.header().representation_authority,
        expected_pre_mint_supply(request, mint.key.to_bytes())?,
    )
}

fn expected_pre_mint_supply(
    request: RepresentationRequestV2<'_>,
    mint: [u8; 32],
) -> Result<u64, ProgramError> {
    let header = request.header();
    let mut expected = if mint == header.receipt_mint {
        Some(header.expected_receipt_supply)
    } else {
        None
    };
    let mut row = 0_u32;
    while row < header.asset_count {
        let asset = request
            .asset(row)
            .map_err(|_| ClaimsSbfError::Instruction)?;
        if asset.shard_mint == mint {
            if expected.is_some() {
                return Err(ClaimsSbfError::Token.into());
            }
            expected = Some(asset.expected_shard_supply);
        }
        row = row.checked_add(1).ok_or(ClaimsSbfError::Token)?;
    }
    expected.ok_or_else(|| ClaimsSbfError::Token.into())
}

fn expected_post_mint_supply(
    prepared: PreparedRepresentationV2<'_>,
    mint: [u8; 32],
) -> Result<u64, ProgramError> {
    let mut expected = expected_pre_mint_supply(prepared.request(), mint)?;
    for effect in prepared.token_effects() {
        let effect = effect.map_err(|_| ClaimsSbfError::Representation)?;
        if effect.mint != mint {
            continue;
        }
        expected = match effect.style {
            TokenEffectStyleV2::MintReceipt | TokenEffectStyleV2::MintShard => expected
                .checked_add(effect.amount)
                .ok_or(ClaimsSbfError::Token)?,
            TokenEffectStyleV2::BurnReceipt | TokenEffectStyleV2::BurnShard => expected
                .checked_sub(effect.amount)
                .ok_or(ClaimsSbfError::Token)?,
            TokenEffectStyleV2::TransferShardToStructured
            | TokenEffectStyleV2::TransferShardFromStructured => expected,
        };
    }
    Ok(expected)
}

#[inline(never)]
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
                let decimals =
                    pre_effect_mint_facts(base, request, accounts.mint)?.display_decimals();
                let instruction = token_instruction::transfer_checked(
                    base.token_program.key,
                    accounts.actor_token.key,
                    accounts.mint.key,
                    accounts.structured_token.key,
                    base.actor.key,
                    &[],
                    effect.amount,
                    decimals,
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
                let decimals =
                    pre_effect_mint_facts(base, request, accounts.mint)?.display_decimals();
                let instruction = token_instruction::transfer_checked(
                    base.token_program.key,
                    accounts.structured_token.key,
                    accounts.mint.key,
                    accounts.actor_token.key,
                    base.representation_authority.key,
                    &[],
                    effect.amount,
                    decimals,
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
                let decimals =
                    pre_effect_mint_facts(base, request, base.receipt_mint)?.display_decimals();
                let instruction = token_instruction::mint_to_checked(
                    base.token_program.key,
                    base.receipt_mint.key,
                    base.actor_receipt.key,
                    base.representation_authority.key,
                    &[],
                    effect.amount,
                    decimals,
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
                let decimals =
                    pre_effect_mint_facts(base, request, base.receipt_mint)?.display_decimals();
                burn(
                    PermissionedBurnContextV2 {
                        program_id: *program_id,
                        base,
                        descriptor: header.descriptor_id,
                        decimals,
                    },
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
                let decimals =
                    pre_effect_mint_facts(base, request, accounts.mint)?.display_decimals();
                burn(
                    PermissionedBurnContextV2 {
                        program_id: *program_id,
                        base,
                        descriptor: header.descriptor_id,
                        decimals,
                    },
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
                let decimals =
                    pre_effect_mint_facts(base, request, accounts.mint)?.display_decimals();
                let instruction = token_instruction::mint_to_checked(
                    base.token_program.key,
                    accounts.mint.key,
                    accounts.actor_token.key,
                    base.representation_authority.key,
                    &[],
                    effect.amount,
                    decimals,
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

#[derive(Clone, Copy)]
struct PermissionedBurnContextV2<'accounts, 'info> {
    program_id: Pubkey,
    base: BaseAccounts<'accounts, 'info>,
    descriptor: [u8; 32],
    decimals: u8,
}

fn burn<'accounts, 'info>(
    context: PermissionedBurnContextV2<'accounts, 'info>,
    source: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    amount: u64,
) -> Result<(), ProgramError> {
    let base = context.base;
    let instruction = permissioned_burn_instruction::burn_checked(
        base.token_program.key,
        source.key,
        mint.key,
        base.representation_authority.key,
        authority.key,
        &[],
        amount,
        context.decimals,
    )
    .map_err(|_| ClaimsSbfError::Token)?;
    let (_, bump) = Pubkey::find_program_address(
        &[
            RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2,
            &context.descriptor,
        ],
        &context.program_id,
    );
    let bump_seed = [bump];
    let signer = [
        RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2,
        context.descriptor.as_slice(),
        bump_seed.as_slice(),
    ];
    invoke_signed(
        &instruction,
        &[
            source.clone(),
            mint.clone(),
            base.representation_authority.clone(),
            authority.clone(),
            base.token_program.clone(),
        ],
        &[&signer],
    )
    .map_err(|_| ClaimsSbfError::Token.into())
}

fn execute_claims_if_any<'accounts, 'info>(
    program_id: &Pubkey,
    account_infos: &'accounts [AccountInfo<'info>],
    base: BaseAccounts<'accounts, 'info>,
    prepared: PreparedRepresentationV2<'_>,
    request_digest: [u8; 32],
    authenticated: Box<AuthenticatedRationalProductV3>,
) -> Result<Box<ClaimsEvidence>, ProgramError> {
    let request = prepared.request();
    if !request.header().action.uses_claims() {
        return Ok(Box::new(ClaimsEvidence {
            plan_digest: [0; 32],
            packet: Vec::new(),
            context: None,
            receipt: None,
            signed_delta_receipt: None,
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
            authenticated,
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
        signed_delta_receipt: None,
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
    authenticated: Box<AuthenticatedRationalProductV3>,
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
    let position = asset_accounts(account_infos, 0)?.position;
    let mut executed = execute_rational_terminal_v3(
        program_id,
        RationalTerminalFrameV3 {
            caller_authority: base.caller_authority,
            aggregate: base.aggregate,
            linked_basis_record: base.linked_basis_record,
            linked_basis_staging: base.linked_basis_staging,
            product_record: base.product_record,
            product_staging: base.product_staging,
            result_domain_record: base.result_domain_record,
            result_domain_staging: base.result_domain_staging,
            portfolio_record: base.portfolio_record,
            portfolio_staging: base.portfolio_staging,
            graph_record: base.graph_raw,
            rent: base.rent,
            core_market: base.core_market,
            cache: base.cache,
            registry: base.registry,
            caller_program: base.caller_program,
            caller_programdata: base.caller_programdata,
            claims_program: base.claims_program,
            claims_programdata: base.claims_programdata,
            core_program: base.core_program,
            core_programdata: base.core_programdata,
            position,
            custody_caller_authority: terminal.caller_authority,
            custody_program: terminal.custody_program,
            coordinate: terminal.coordinate,
            coordinate_staging: terminal.coordinate_staging,
            realm: terminal.realm,
            realm_staging: terminal.realm_staging,
            custody_replay: terminal.replay,
            collateral_mint: terminal.collateral_mint,
            hoard: terminal.hoard,
            recipient: terminal.recipient,
            custody_authority: terminal.custody_authority,
            token_program: terminal.token_program,
        },
        request,
        request_digest,
        authenticated,
    )?;
    let custody = executed.custody.take().map(|evidence| {
        Box::new(CustodyEvidence {
            request: evidence.request,
            request_digest: evidence.request_digest,
            receipt: evidence.receipt,
            receipt_digest: evidence.receipt_digest,
            replay_digest: evidence.replay_digest,
        })
    });
    Ok(Box::new(ClaimsEvidence {
        plan_digest: executed.packet_digest,
        packet: core::mem::take(&mut executed.packet),
        context: None,
        receipt: None,
        signed_delta_receipt: Some(executed.receipt),
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

#[inline(never)]
fn post_token_observations(
    account_infos: &[AccountInfo<'_>],
    base: BaseAccounts<'_, '_>,
    prepared: PreparedRepresentationV2<'_>,
) -> Result<(Vec<u8>, u64), ProgramError> {
    let request = prepared.request();
    let header = request.header();
    let receipt = parse_behavior_mint(
        base.receipt_mint,
        base.token_program,
        header.receipt_mint,
        header.representation_authority,
        expected_post_mint_supply(prepared, header.receipt_mint)?,
    )?
    .mint();
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
        let mint = parse_behavior_mint(
            accounts.mint,
            base.token_program,
            requested.shard_mint,
            header.representation_authority,
            expected_post_mint_supply(prepared, requested.shard_mint)?,
        )?
        .mint();
        let actor = parse_behavior_token_account(
            accounts.actor_token,
            base.token_program,
            requested.shard_mint,
            header.actor,
            0,
        )?
        .account();
        let structured = parse_behavior_token_account(
            accounts.structured_token,
            base.token_program,
            requested.shard_mint,
            header.representation_authority,
            0,
        )?
        .account();
        output.extend_from_slice(&mint.supply.to_le_bytes());
        output.extend_from_slice(&actor.amount.to_le_bytes());
        output.extend_from_slice(&structured.amount.to_le_bytes());
        row = row.checked_add(1).ok_or(ClaimsSbfError::Token)?;
    }
    Ok((output, receipt.supply))
}

#[inline(never)]
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

fn parse_behavior_mint(
    account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    expected: [u8; 32],
    expected_controller: [u8; 32],
    expected_base_supply: u64,
) -> Result<Token2022BehaviorMintFactsV2, ProgramError> {
    if account.key.to_bytes() != expected || account.owner != token_program.key {
        return Err(ClaimsSbfError::Token.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    Token2022BehaviorProfileV2::check_mint(
        token_program.key.to_bytes(),
        account.key.to_bytes(),
        &data,
        expected_controller,
        expected_base_supply,
    )
    .map_err(|_| ClaimsSbfError::Token.into())
}

fn parse_behavior_token_account(
    account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    mint: [u8; 32],
    owner: [u8; 32],
    minimum_base_amount: u64,
) -> Result<Token2022BehaviorAccountFactsV2, ProgramError> {
    if account.owner != token_program.key {
        return Err(ClaimsSbfError::Token.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    Token2022BehaviorProfileV2::check_account(
        token_program.key.to_bytes(),
        &data,
        mint,
        owner,
        minimum_base_amount,
    )
    .map_err(|_| ClaimsSbfError::Token.into())
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
