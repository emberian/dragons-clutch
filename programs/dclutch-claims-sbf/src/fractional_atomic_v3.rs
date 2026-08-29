//! Claims-owned atomic Fractional native-Claims and Token-2022 execution.
//!
//! Open and terminal actions use distinct exact frames. The enclosing activated
//! Trading program signs both its release-pinned caller authority and the
//! terms/Market-bound Fractional root PDA. Claims then commits the canonical
//! two-Position SignedDelta before invoking Token-2022; any later Token or
//! postcondition refusal rolls the complete SVM instruction back.

extern crate alloc;

use alloc::{boxed::Box, vec};

use dclutch_claims_svm::{
    CallerRole,
    liability_basis_state_v2::{LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2},
    signed_delta_v3::{PositionDeltaV3, SignedDeltaReceiptV3, SignedDeltaV3},
};
use dclutch_custody_contract::CustodyReplayV1;
use dclutch_fractional_claim_contract::{
    FRACTIONAL_ATOMIC_ACCOUNT_COUNT_V3, FRACTIONAL_ATOMIC_ACTOR_V3,
    FRACTIONAL_ATOMIC_HOLDER_TOKEN_V3, FRACTIONAL_ATOMIC_ROOT_V3, FRACTIONAL_ATOMIC_SHARD_MINT_V3,
    FRACTIONAL_ATOMIC_SIGNED_DELTA_ACCOUNT_COUNT_V3, FRACTIONAL_ATOMIC_TERMS_RAW_V3,
    FRACTIONAL_ATOMIC_TERMS_STAGING_V3, FRACTIONAL_ATOMIC_TOKEN_BEHAVIOR_RAW_V3,
    FRACTIONAL_ATOMIC_TOKEN_BEHAVIOR_STAGING_V3, FRACTIONAL_ATOMIC_TOKEN_PROGRAM_V3,
    FRACTIONAL_TERMINAL_ACCOUNT_COUNT_V3, FRACTIONAL_TERMINAL_ACTOR_V3,
    FRACTIONAL_TERMINAL_ATOMIC_RECEIPT_BYTES_V3, FRACTIONAL_TERMINAL_BASE_ACCOUNT_COUNT_V3,
    FRACTIONAL_TERMINAL_ROOT_V3, FRACTIONAL_TERMINAL_SHARD_MINT_V3,
    FRACTIONAL_TERMINAL_SOURCE_TOKEN_V3, FRACTIONAL_TERMINAL_TERMS_RAW_V3,
    FRACTIONAL_TERMINAL_TERMS_STAGING_V3, FRACTIONAL_TERMINAL_TOKEN_BEHAVIOR_RAW_V3,
    FRACTIONAL_TERMINAL_TOKEN_BEHAVIOR_STAGING_V3, FractionalAtomicReceiptV3,
    FractionalExposureActionV2, FractionalExposureRequestV2, FractionalTerminalAtomicReceiptV3,
    decode_fractional_capability_root_v4, plan_fractional_physical_v3,
};
use dclutch_fractional_claim_kernel::{
    FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2, FractionalExposureTermsAdmissionV2,
    FractionalExposureTermsV2,
};
use dclutch_fractional_claims_kernel::{
    FractionalExposureSignedDeltaInputV2, PreparedFractionalExposureSignedDeltaV2,
    fractional_exposure_signed_delta_shape_v2, prepare_fractional_exposure_signed_delta_v2,
    validate_fractional_exposure_signed_delta_postcondition_v2,
};
use dclutch_sha256_adapter::{digest, digestv};
use dclutch_token_svm::{
    MINT_BYTES, Mint, TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2, Token2022BehaviorProfileV2,
    TokenBehaviorSelectionV2,
};
use solana_program::{
    account_info::AccountInfo,
    program::{invoke, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use spl_token_2022_interface::{
    extension::permissioned_burn::instruction as permissioned_burn_instruction,
    instruction as token_instruction,
};

use crate::{
    ClaimsSbfError,
    rational_representation_v2::authenticate_finalized_rational_record,
    signed_delta_v3::{
        AuthenticatedSignedDeltaParentV3, ParentAuthorityV3, authenticate_parent_releases,
        execute_parent_authenticated,
    },
    terminal_settlement_v3::execute_enclosing_authenticated as execute_terminal_enclosing,
};

const MARKET: usize = 1;
const RENT: usize = 10;
const REGISTRY: usize = 13;
const TRADING_PROGRAM: usize = 14;
const CLAIMS_PROGRAM: usize = 16;
const POSITION_0: usize = 20;
const POSITION_1: usize = 21;
const TOKEN_POST_DOMAIN: &[u8] = b"dclutch/fractional-atomic-token-post/v3";

/// Execute one exact atomic open-market Fractional action.
#[inline(never)]
pub(super) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let request = Box::new(
        FractionalExposureRequestV2::decode(instruction_data)
            .map_err(|_| ClaimsSbfError::Instruction)?,
    );
    match request.action() {
        FractionalExposureActionV2::Wrap | FractionalExposureActionV2::WholeUnwrap => {
            process_open(program_id, accounts, instruction_data, &request)
        }
        FractionalExposureActionV2::TerminalRedeem
        | FractionalExposureActionV2::TerminalZeroBurn => {
            process_terminal(program_id, accounts, instruction_data, &request)
        }
        _ => Err(ClaimsSbfError::Instruction.into()),
    }
}

#[inline(never)]
fn execute_signed_delta_boxed(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    packet: &[u8],
    parent: AuthenticatedSignedDeltaParentV3,
) -> Result<Box<SignedDeltaReceiptV3>, ProgramError> {
    Ok(Box::new(execute_parent_authenticated(
        program_id, accounts, packet, parent,
    )?))
}

#[inline(never)]
fn process_open(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
    request: &FractionalExposureRequestV2,
) -> Result<(), ProgramError> {
    if accounts.len() != FRACTIONAL_ATOMIC_ACCOUNT_COUNT_V3 {
        return Err(ClaimsSbfError::Accounts.into());
    }
    authenticate_tail_privileges(program_id, accounts, *request)?;
    let rent =
        Rent::from_account_info(account(accounts, RENT)?).map_err(|_| ClaimsSbfError::Accounts)?;
    let registry = account(accounts, REGISTRY)?;

    let terms_raw = account(accounts, FRACTIONAL_ATOMIC_TERMS_RAW_V3)?;
    let terms_staging = account(accounts, FRACTIONAL_ATOMIC_TERMS_STAGING_V3)?;
    let terms_data = terms_raw
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    authenticate_finalized_rational_record(
        registry.key,
        &rent,
        terms_raw,
        terms_staging,
        FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
        request.input().terms,
        &terms_data,
    )?;
    let terms = FractionalExposureTermsV2::decode(
        &terms_data,
        FractionalExposureTermsAdmissionV2 {
            selected_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            finalized_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            selected_terms_id: request.input().terms,
            finalized_terms_id: request.input().terms,
            recomputed_terms_digest: request.input().terms,
            finalized_terms_digest: request.input().terms,
            record_authenticated: true,
        },
    )
    .map_err(|_| ClaimsSbfError::Representation)?;
    (*request)
        .bind_terms(terms)
        .map_err(|_| ClaimsSbfError::Representation)?;

    let market_data = account(accounts, MARKET)?
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let market =
        LiabilityBasisMarketViewV2::decode(&market_data).map_err(|_| ClaimsSbfError::Identity)?;
    if market.realm_id == [0; 32] {
        return Err(ClaimsSbfError::Identity.into());
    }

    let behavior_raw = account(accounts, FRACTIONAL_ATOMIC_TOKEN_BEHAVIOR_RAW_V3)?;
    let behavior_staging = account(accounts, FRACTIONAL_ATOMIC_TOKEN_BEHAVIOR_STAGING_V3)?;
    let behavior_data = behavior_raw
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    authenticate_finalized_rational_record(
        registry.key,
        &rent,
        behavior_raw,
        behavior_staging,
        TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
        request.input().token_behavior,
        &behavior_data,
    )?;
    let behavior = TokenBehaviorSelectionV2::decode_for_authenticated_selection(
        &behavior_data,
        market.realm_id,
        request.input().release_set,
    )
    .map_err(|_| ClaimsSbfError::Token)?;
    if behavior.token_program() != terms.token_program() {
        return Err(ClaimsSbfError::Token.into());
    }

    let root_account = account(accounts, FRACTIONAL_ATOMIC_ROOT_V3)?;
    let root_data = root_account
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let composite_root =
        decode_fractional_capability_root_v4(&root_data).ok_or(ClaimsSbfError::Representation)?;
    let root = composite_root.state();
    let root_input = root.input();
    let trading_program = account(accounts, TRADING_PROGRAM)?;
    let header = composite_root.header();
    let (expected_root, expected_bump) =
        Pubkey::find_program_address(&header.seeds().as_slices(), trading_program.key);
    if root_account.key != &expected_root
        || root_account.owner != trading_program.key
        || header.release_set().to_bytes() != request.input().release_set
        || header.market() != request.input().market
        || header.selection().config().to_bytes() != request.input().terms
        || root_input.bump != expected_bump
        || root_input.terms != request.input().terms
        || root_input.market != request.input().market
        || root_input.revision != request.input().expected_revision
    {
        return Err(ClaimsSbfError::Representation.into());
    }
    drop(root_data);

    let physical =
        plan_fractional_physical_v3(terms, *request).map_err(|_| ClaimsSbfError::Representation)?;
    let mint_account = account(accounts, FRACTIONAL_ATOMIC_SHARD_MINT_V3)?;
    let holder_account = account(accounts, FRACTIONAL_ATOMIC_HOLDER_TOKEN_V3)?;
    let token_program = account(accounts, FRACTIONAL_ATOMIC_TOKEN_PROGRAM_V3)?;
    if physical.shard_mint != Some(mint_account.key.to_bytes()) {
        return Err(ClaimsSbfError::Token.into());
    }
    if token_program.key.to_bytes() != terms.token_program()
        || mint_account.owner != token_program.key
        || holder_account.owner != token_program.key
    {
        return Err(ClaimsSbfError::Token.into());
    }
    let mint_data = mint_account
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let mint_base = Mint::parse(mint_data.get(..MINT_BYTES).ok_or(ClaimsSbfError::Token)?)
        .map_err(|_| ClaimsSbfError::Token)?;
    let mint_facts = Token2022BehaviorProfileV2::check_mint(
        terms.token_program(),
        mint_account.key.to_bytes(),
        &mint_data,
        root_account.key.to_bytes(),
        mint_base.supply,
    )
    .map_err(|_| ClaimsSbfError::Token)?;
    let pre_supply = mint_facts.base_supply();
    let decimals = mint_facts.display_decimals();
    drop(mint_data);
    let holder_data = holder_account
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let holder_facts = Token2022BehaviorProfileV2::check_account(
        terms.token_program(),
        &holder_data,
        physical.shard_mint.ok_or(ClaimsSbfError::Token)?,
        request.input().owner,
        if request.action() == FractionalExposureActionV2::WholeUnwrap {
            physical.consumed_shards
        } else {
            0
        },
    )
    .map_err(|_| ClaimsSbfError::Token)?;
    let pre_holder = holder_facts.base_amount();
    drop(holder_data);

    let position_0_data = account(accounts, POSITION_0)?
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let position_1_data = account(accounts, POSITION_1)?
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let position_0 = LiabilityBasisPositionViewV2::decode(&position_0_data)
        .map_err(|_| ClaimsSbfError::Identity)?;
    let position_1 = LiabilityBasisPositionViewV2::decode(&position_1_data)
        .map_err(|_| ClaimsSbfError::Identity)?;
    let (reserve_position, actor_position) = if position_0.owner == root_account.key.to_bytes()
        && position_1.owner == request.input().owner
    {
        (&*position_0_data, &*position_1_data)
    } else if position_1.owner == root_account.key.to_bytes()
        && position_0.owner == request.input().owner
    {
        (&*position_1_data, &*position_0_data)
    } else {
        return Err(ClaimsSbfError::Identity.into());
    };
    let input = FractionalExposureSignedDeltaInputV2 {
        request: *request,
        terms,
        semantic_product_id: market.product_instance_id,
        market_account: account(accounts, MARKET)?.key.to_bytes(),
        market_bytes: &market_data,
        claims_program: program_id.to_bytes(),
        reserve_owner: root_account.key.to_bytes(),
        reserve_position_bytes: reserve_position,
        actor_position_bytes: actor_position,
    };
    let shape =
        fractional_exposure_signed_delta_shape_v2(input).map_err(|_| ClaimsSbfError::Economic)?;
    let claim_count = usize::try_from(shape.claim_count()).map_err(|_| ClaimsSbfError::Economic)?;
    let mut aggregate_scratch = vec![
        SignedDeltaV3::new(
            dclutch_claims_svm::signed_delta_v3::DeltaDirectionV3::Neutral,
            0
        )
        .map_err(|_| ClaimsSbfError::Economic)?;
        claim_count
    ];
    let placeholder = PositionDeltaV3::new(
        dclutch_claims_svm::signed_delta_v3::PositionDeltaInputV3 {
            position_index: 0,
            outcome: 0,
            delta: SignedDeltaV3::new(
                dclutch_claims_svm::signed_delta_v3::DeltaDirectionV3::Credit,
                1,
            )
            .map_err(|_| ClaimsSbfError::Economic)?,
        },
        2,
        shape.claim_count(),
    )
    .map_err(|_| ClaimsSbfError::Economic)?;
    let mut row_scratch = [placeholder; 2];
    let mut packet = vec![0; shape.packet_bytes()];
    let prepared = Box::new(
        prepare_fractional_exposure_signed_delta_v2(
            input,
            &mut aggregate_scratch,
            &mut row_scratch,
            &mut packet,
        )
        .map_err(|_| ClaimsSbfError::Economic)?,
    );
    drop(position_0_data);
    drop(position_1_data);
    drop(market_data);
    drop(behavior_data);

    let signed_accounts = accounts
        .get(..FRACTIONAL_ATOMIC_SIGNED_DELTA_ACCOUNT_COUNT_V3)
        .ok_or(ClaimsSbfError::Accounts)?;
    authenticate_parent_releases(program_id, signed_accounts, &packet)?;
    let request_digest = digest(instruction_data);
    let signed_receipt = execute_signed_delta_boxed(
        program_id,
        signed_accounts,
        &packet,
        AuthenticatedSignedDeltaParentV3 {
            caller_role: CallerRole::Trading,
            authority: ParentAuthorityV3::CallerProgramPda,
            release_set: request.input().release_set,
            market: request.input().market,
            parent_context: request.input().terms,
            parent_request_digest: request_digest,
        },
    )?;

    execute_token(
        *request,
        physical.consumed_shards,
        decimals,
        root_account,
        account(accounts, FRACTIONAL_ATOMIC_ACTOR_V3)?,
        mint_account,
        holder_account,
        token_program,
    )?;

    let expected_supply = match request.action() {
        FractionalExposureActionV2::Wrap => pre_supply.checked_add(physical.consumed_shards),
        FractionalExposureActionV2::WholeUnwrap => pre_supply.checked_sub(physical.consumed_shards),
        _ => None,
    }
    .ok_or(ClaimsSbfError::Token)?;
    let expected_holder = match request.action() {
        FractionalExposureActionV2::Wrap => pre_holder.checked_add(physical.consumed_shards),
        FractionalExposureActionV2::WholeUnwrap => pre_holder.checked_sub(physical.consumed_shards),
        _ => None,
    }
    .ok_or(ClaimsSbfError::Token)?;
    let post_mint = mint_account
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    Token2022BehaviorProfileV2::check_mint(
        terms.token_program(),
        mint_account.key.to_bytes(),
        &post_mint,
        root_account.key.to_bytes(),
        expected_supply,
    )
    .map_err(|_| ClaimsSbfError::Token)?;
    let post_holder = holder_account
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    Token2022BehaviorProfileV2::check_account(
        terms.token_program(),
        &post_holder,
        physical.shard_mint.ok_or(ClaimsSbfError::Token)?,
        request.input().owner,
        expected_holder,
    )
    .map_err(|_| ClaimsSbfError::Token)?;
    let exact_holder =
        dclutch_token_svm::TokenAccount::parse(&post_holder).map_err(|_| ClaimsSbfError::Token)?;
    if exact_holder.amount != expected_holder {
        return Err(ClaimsSbfError::Token.into());
    }
    let token_post_digest = digestv(&[TOKEN_POST_DOMAIN, &post_mint, &post_holder]);

    let post_market = account(accounts, MARKET)?
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let post_position_0 = account(accounts, POSITION_0)?
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let post_position_1 = account(accounts, POSITION_1)?
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    validate_open_and_emit(
        request,
        request_digest,
        &prepared,
        &packet,
        &signed_receipt,
        token_post_digest,
        root_account.key.to_bytes(),
        expected_supply,
        expected_holder,
        physical.consumed_shards,
        position_0.owner,
        &post_market,
        &post_position_0,
        &post_position_1,
    )
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn validate_open_and_emit(
    request: &FractionalExposureRequestV2,
    request_digest: [u8; 32],
    prepared: &PreparedFractionalExposureSignedDeltaV2,
    packet: &[u8],
    signed_receipt: &SignedDeltaReceiptV3,
    token_post_digest: [u8; 32],
    root: [u8; 32],
    expected_supply: u64,
    expected_holder: u64,
    consumed_shards: u64,
    position_0_owner: [u8; 32],
    post_market: &[u8],
    post_position_0: &[u8],
    post_position_1: &[u8],
) -> Result<(), ProgramError> {
    let signed_receipt_bytes = signed_receipt.to_bytes();
    validate_fractional_exposure_signed_delta_postcondition_v2(
        *prepared,
        packet,
        signed_receipt.packet_digest(),
        signed_receipt.table_digest(),
        signed_receipt.post_resource_digest(),
        &signed_receipt_bytes,
        post_market,
        &[post_position_0, post_position_1],
    )
    .map_err(|_| ClaimsSbfError::Receipt)?;
    let actor_view = if position_0_owner == request.input().owner {
        LiabilityBasisPositionViewV2::decode(post_position_0)
    } else {
        LiabilityBasisPositionViewV2::decode(post_position_1)
    }
    .map_err(|_| ClaimsSbfError::Receipt)?;
    let reserve_view = if position_0_owner == root {
        LiabilityBasisPositionViewV2::decode(post_position_0)
    } else {
        LiabilityBasisPositionViewV2::decode(post_position_1)
    }
    .map_err(|_| ClaimsSbfError::Receipt)?;
    let receipt = FractionalAtomicReceiptV3::new(
        request.action(),
        request_digest,
        signed_receipt.packet_digest(),
        digest(&signed_receipt_bytes),
        token_post_digest,
        signed_receipt.post_resource_digest(),
        root,
        signed_receipt.post_market_revision(),
        actor_view.revision,
        reserve_view.revision,
        expected_supply,
        expected_holder,
        consumed_shards,
    )
    .map_err(|_| ClaimsSbfError::Receipt)?;
    set_return_data(&receipt.to_bytes());
    Ok(())
}

#[inline(never)]
fn process_terminal(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
    request: &FractionalExposureRequestV2,
) -> Result<(), ProgramError> {
    use dclutch_claims_svm::terminal_settlement_v3::{
        TERMINAL_SETTLEMENT_COLLATERAL_MINT_ACCOUNT_V3 as COLLATERAL_MINT,
        TERMINAL_SETTLEMENT_CUSTODY_PROGRAM_ACCOUNT_V3 as CUSTODY_PROGRAM,
        TERMINAL_SETTLEMENT_CUSTODY_REPLAY_ACCOUNT_V3 as CUSTODY_REPLAY,
        TERMINAL_SETTLEMENT_EXPOSURE_RAW_ACCOUNT_V3 as EXPOSURE_RAW,
        TERMINAL_SETTLEMENT_RECIPIENT_ACCOUNT_V3 as RECIPIENT,
        TERMINAL_SETTLEMENT_TOKEN_PROGRAM_ACCOUNT_V3 as TOKEN_PROGRAM,
        TerminalSettlementRequestInputV3, TerminalSettlementRequestV3,
    };
    if accounts.len() != FRACTIONAL_TERMINAL_ACCOUNT_COUNT_V3 {
        return Err(ClaimsSbfError::Accounts.into());
    }
    authenticate_terminal_tail_privileges(program_id, accounts, *request)?;
    let rent =
        Rent::from_account_info(account(accounts, RENT)?).map_err(|_| ClaimsSbfError::Accounts)?;
    let registry = account(accounts, REGISTRY)?;
    let terms_raw = account(accounts, FRACTIONAL_TERMINAL_TERMS_RAW_V3)?;
    let terms_staging = account(accounts, FRACTIONAL_TERMINAL_TERMS_STAGING_V3)?;
    let terms_data = terms_raw
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    authenticate_finalized_rational_record(
        registry.key,
        &rent,
        terms_raw,
        terms_staging,
        FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
        request.input().terms,
        &terms_data,
    )?;
    let terms = FractionalExposureTermsV2::decode(
        &terms_data,
        FractionalExposureTermsAdmissionV2 {
            selected_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            finalized_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            selected_terms_id: request.input().terms,
            finalized_terms_id: request.input().terms,
            recomputed_terms_digest: request.input().terms,
            finalized_terms_digest: request.input().terms,
            record_authenticated: true,
        },
    )
    .map_err(|_| ClaimsSbfError::Representation)?;
    (*request)
        .bind_terms(terms)
        .map_err(|_| ClaimsSbfError::Representation)?;

    let market_data = account(accounts, MARKET)?
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let market =
        LiabilityBasisMarketViewV2::decode(&market_data).map_err(|_| ClaimsSbfError::Identity)?;
    if market.logical_market != request.input().market
        || market.release_set != request.input().release_set
        || market.realm_id == [0; 32]
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    let behavior_raw = account(accounts, FRACTIONAL_TERMINAL_TOKEN_BEHAVIOR_RAW_V3)?;
    let behavior_staging = account(accounts, FRACTIONAL_TERMINAL_TOKEN_BEHAVIOR_STAGING_V3)?;
    let behavior_data = behavior_raw
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    authenticate_finalized_rational_record(
        registry.key,
        &rent,
        behavior_raw,
        behavior_staging,
        TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
        request.input().token_behavior,
        &behavior_data,
    )?;
    let behavior = TokenBehaviorSelectionV2::decode_for_authenticated_selection(
        &behavior_data,
        market.realm_id,
        request.input().release_set,
    )
    .map_err(|_| ClaimsSbfError::Token)?;
    if behavior.token_program() != terms.token_program() {
        return Err(ClaimsSbfError::Token.into());
    }

    let root_account = account(accounts, FRACTIONAL_TERMINAL_ROOT_V3)?;
    let root_data = root_account
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let composite_root =
        decode_fractional_capability_root_v4(&root_data).ok_or(ClaimsSbfError::Representation)?;
    let root = composite_root.state();
    let root_input = root.input();
    let trading_program = account(accounts, TRADING_PROGRAM)?;
    let header = composite_root.header();
    let (expected_root, expected_bump) =
        Pubkey::find_program_address(&header.seeds().as_slices(), trading_program.key);
    if root_account.key != &expected_root
        || root_account.owner != trading_program.key
        || header.release_set().to_bytes() != request.input().release_set
        || header.market() != request.input().market
        || header.selection().config().to_bytes() != request.input().terms
        || root_input.bump != expected_bump
        || root_input.terms != request.input().terms
        || root_input.market != request.input().market
        || root_input.revision != request.input().expected_revision
    {
        return Err(ClaimsSbfError::Representation.into());
    }
    drop(root_data);

    let physical =
        plan_fractional_physical_v3(terms, *request).map_err(|_| ClaimsSbfError::Representation)?;
    let mint = account(accounts, FRACTIONAL_TERMINAL_SHARD_MINT_V3)?;
    let source = account(accounts, FRACTIONAL_TERMINAL_SOURCE_TOKEN_V3)?;
    let token_program = account(accounts, TOKEN_PROGRAM)?;
    if physical.shard_mint != Some(mint.key.to_bytes())
        || token_program.key.to_bytes() != terms.token_program()
        || mint.owner != token_program.key
        || source.owner != token_program.key
    {
        return Err(ClaimsSbfError::Token.into());
    }
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let mint_base = Mint::parse(mint_data.get(..MINT_BYTES).ok_or(ClaimsSbfError::Token)?)
        .map_err(|_| ClaimsSbfError::Token)?;
    let mint_facts = Token2022BehaviorProfileV2::check_mint(
        terms.token_program(),
        mint.key.to_bytes(),
        &mint_data,
        root_account.key.to_bytes(),
        mint_base.supply,
    )
    .map_err(|_| ClaimsSbfError::Token)?;
    let pre_supply = mint_facts.base_supply();
    let decimals = mint_facts.display_decimals();
    drop(mint_data);
    let source_data = source
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let source_facts = Token2022BehaviorProfileV2::check_account(
        terms.token_program(),
        &source_data,
        mint.key.to_bytes(),
        request.input().owner,
        physical.consumed_shards,
    )
    .map_err(|_| ClaimsSbfError::Token)?;
    let pre_holder = source_facts.base_amount();
    drop(source_data);

    let position_data = account(accounts, POSITION_0)?
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let position = LiabilityBasisPositionViewV2::decode(&position_data)
        .map_err(|_| ClaimsSbfError::Identity)?;
    if position.owner != root_account.key.to_bytes()
        || position.market_account != account(accounts, MARKET)?.key.to_bytes()
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    let replay_data = account(accounts, CUSTODY_REPLAY)?
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let replay = CustodyReplayV1::decode(&replay_data).map_err(|_| ClaimsSbfError::Identity)?;
    if replay.release_set != request.input().release_set
        || replay.market != request.input().market
        || replay.realm != market.realm_id
        || replay.generation != market.generation
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    let exposure_data = account(accounts, EXPOSURE_RAW)?
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let basis_data = account(accounts, 2)?
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let outer_digest = digest(instruction_data);
    let terminal_request = Box::new(
        TerminalSettlementRequestV3::new(TerminalSettlementRequestInputV3 {
            caller_role: CallerRole::Trading,
            release_set: request.input().release_set,
            market: request.input().market,
            realm: market.realm_id,
            parent_context: outer_digest,
            product_record_digest: request.input().product_record,
            exposure_id: request.input().exposure,
            exposure_digest: digest(&exposure_data),
            terminal_record_digest: request.input().terminal_digest,
            owner: root_account.key.to_bytes(),
            position: account(accounts, POSITION_0)?.key.to_bytes(),
            recipient_owner: request.input().owner,
            recipient_token_account: account(accounts, RECIPIENT)?.key.to_bytes(),
            claims_program: program_id.to_bytes(),
            custody_program: account(accounts, CUSTODY_PROGRAM)?.key.to_bytes(),
            collateral_mint: account(accounts, COLLATERAL_MINT)?.key.to_bytes(),
            token_program: token_program.key.to_bytes(),
            semantic_basis_id: terms.representation_basis(),
            linked_basis_record_digest: digest(&basis_data),
            generation: market.generation,
            expected_market_revision: market.revision,
            expected_position_revision: position.revision,
            expected_custody_revision: replay.next_revision,
            quantity: physical.whole_claims,
            claim_index: request.input().representation_coordinate,
            transfer_index: 0,
        })
        .map_err(|_| ClaimsSbfError::Instruction)?,
    );
    drop(basis_data);
    drop(exposure_data);
    drop(replay_data);
    drop(position_data);
    drop(behavior_data);
    drop(market_data);

    let terminal_request_digest = terminal_request_digest(&terminal_request);
    let terminal_receipt = execute_terminal_boxed(
        program_id,
        accounts
            .get(..FRACTIONAL_TERMINAL_BASE_ACCOUNT_COUNT_V3)
            .ok_or(ClaimsSbfError::Accounts)?,
        *terminal_request,
        request.input().terms,
        outer_digest,
    )?;
    let terminal_evidence = terminal_receipt.evidence();
    if (request.action() == FractionalExposureActionV2::TerminalRedeem)
        != (terminal_evidence.payout != 0)
    {
        return Err(ClaimsSbfError::Economic.into());
    }
    execute_terminal_burn(
        physical.consumed_shards,
        decimals,
        root_account,
        account(accounts, FRACTIONAL_TERMINAL_ACTOR_V3)?,
        mint,
        source,
        token_program,
    )?;
    let expected_supply = pre_supply
        .checked_sub(physical.consumed_shards)
        .ok_or(ClaimsSbfError::Token)?;
    let expected_holder = pre_holder
        .checked_sub(physical.consumed_shards)
        .ok_or(ClaimsSbfError::Token)?;
    let post_mint = mint
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    Token2022BehaviorProfileV2::check_mint(
        terms.token_program(),
        mint.key.to_bytes(),
        &post_mint,
        root_account.key.to_bytes(),
        expected_supply,
    )
    .map_err(|_| ClaimsSbfError::Token)?;
    let post_source = source
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let post = Token2022BehaviorProfileV2::check_account(
        terms.token_program(),
        &post_source,
        mint.key.to_bytes(),
        request.input().owner,
        expected_holder,
    )
    .map_err(|_| ClaimsSbfError::Token)?;
    if post.base_amount() != expected_holder {
        return Err(ClaimsSbfError::Token.into());
    }
    let token_post_digest = digestv(&[TOKEN_POST_DOMAIN, &post_mint, &post_source]);
    emit_terminal_receipt(
        request.action(),
        outer_digest,
        terminal_request_digest,
        &terminal_receipt,
        token_post_digest,
        root_account.key.to_bytes(),
        expected_supply,
        expected_holder,
        physical.consumed_shards,
    )
}

#[inline(never)]
fn terminal_request_digest(
    request: &dclutch_claims_svm::terminal_settlement_v3::TerminalSettlementRequestV3,
) -> [u8; 32] {
    digest(&request.to_bytes())
}

#[inline(never)]
fn execute_terminal_boxed(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: dclutch_claims_svm::terminal_settlement_v3::TerminalSettlementRequestV3,
    outer_context: [u8; 32],
    outer_request_digest: [u8; 32],
) -> Result<
    Box<dclutch_claims_svm::terminal_settlement_v3::TerminalSettlementReceiptV3>,
    ProgramError,
> {
    Ok(Box::new(execute_terminal_enclosing(
        program_id,
        accounts,
        request,
        outer_context,
        outer_request_digest,
    )?))
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn emit_terminal_receipt(
    action: FractionalExposureActionV2,
    request_digest: [u8; 32],
    terminal_request_digest: [u8; 32],
    terminal_receipt: &dclutch_claims_svm::terminal_settlement_v3::TerminalSettlementReceiptV3,
    token_post_digest: [u8; 32],
    root: [u8; 32],
    post_mint_supply: u64,
    post_holder_amount: u64,
    consumed_shards: u64,
) -> Result<(), ProgramError> {
    let terminal_evidence = terminal_receipt.evidence();
    let terminal_bytes = terminal_receipt.to_bytes();
    let receipt = FractionalTerminalAtomicReceiptV3::new(
        action,
        request_digest,
        terminal_request_digest,
        digest(&terminal_bytes),
        terminal_evidence.post_resource_digest,
        token_post_digest,
        root,
        terminal_evidence.payout,
        post_mint_supply,
        post_holder_amount,
        consumed_shards,
    )
    .map_err(|_| ClaimsSbfError::Receipt)?;
    let receipt_bytes = receipt.to_bytes();
    if receipt_bytes.len() != FRACTIONAL_TERMINAL_ATOMIC_RECEIPT_BYTES_V3 {
        return Err(ClaimsSbfError::Receipt.into());
    }
    set_return_data(&receipt_bytes);
    Ok(())
}

fn execute_terminal_burn<'info>(
    amount: u64,
    decimals: u8,
    root: &AccountInfo<'info>,
    actor: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    source: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
) -> Result<(), ProgramError> {
    let instruction = permissioned_burn_instruction::burn_checked(
        token_program.key,
        source.key,
        mint.key,
        root.key,
        actor.key,
        &[],
        amount,
        decimals,
    )
    .map_err(|_| ClaimsSbfError::Token)?;
    invoke(
        &instruction,
        &[
            source.clone(),
            mint.clone(),
            root.clone(),
            actor.clone(),
            token_program.clone(),
        ],
    )
    .map_err(|_| ClaimsSbfError::Token.into())
}

#[allow(clippy::too_many_arguments)]
fn execute_token<'info>(
    request: FractionalExposureRequestV2,
    amount: u64,
    decimals: u8,
    root: &AccountInfo<'info>,
    actor: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    holder: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
) -> Result<(), ProgramError> {
    let instruction = match request.action() {
        FractionalExposureActionV2::Wrap => token_instruction::mint_to_checked(
            token_program.key,
            mint.key,
            holder.key,
            root.key,
            &[],
            amount,
            decimals,
        ),
        FractionalExposureActionV2::WholeUnwrap => permissioned_burn_instruction::burn_checked(
            token_program.key,
            holder.key,
            mint.key,
            root.key,
            actor.key,
            &[],
            amount,
            decimals,
        ),
        _ => return Err(ClaimsSbfError::Instruction.into()),
    }
    .map_err(|_| ClaimsSbfError::Token)?;
    let infos = match request.action() {
        FractionalExposureActionV2::Wrap => vec![
            mint.clone(),
            holder.clone(),
            root.clone(),
            token_program.clone(),
        ],
        FractionalExposureActionV2::WholeUnwrap => vec![
            holder.clone(),
            mint.clone(),
            root.clone(),
            actor.clone(),
            token_program.clone(),
        ],
        _ => return Err(ClaimsSbfError::Instruction.into()),
    };
    invoke(&instruction, &infos).map_err(|_| ClaimsSbfError::Token.into())
}

fn authenticate_tail_privileges(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: FractionalExposureRequestV2,
) -> Result<(), ProgramError> {
    let root = account(accounts, FRACTIONAL_ATOMIC_ROOT_V3)?;
    let actor = account(accounts, FRACTIONAL_ATOMIC_ACTOR_V3)?;
    let mint = account(accounts, FRACTIONAL_ATOMIC_SHARD_MINT_V3)?;
    let holder = account(accounts, FRACTIONAL_ATOMIC_HOLDER_TOKEN_V3)?;
    let token = account(accounts, FRACTIONAL_ATOMIC_TOKEN_PROGRAM_V3)?;
    for index in [
        FRACTIONAL_ATOMIC_TERMS_RAW_V3,
        FRACTIONAL_ATOMIC_TERMS_STAGING_V3,
        FRACTIONAL_ATOMIC_TOKEN_BEHAVIOR_RAW_V3,
        FRACTIONAL_ATOMIC_TOKEN_BEHAVIOR_STAGING_V3,
    ] {
        let value = account(accounts, index)?;
        if value.is_writable || value.is_signer || value.executable {
            return Err(ClaimsSbfError::Accounts.into());
        }
    }
    if !root.is_signer
        || !root.is_writable
        || root.executable
        || !actor.is_signer
        || actor.is_writable
        || actor.executable
        || actor.key.to_bytes() != request.input().owner
        || !mint.is_writable
        || mint.is_signer
        || mint.executable
        || !holder.is_writable
        || holder.is_signer
        || holder.executable
        || holder.key.to_bytes()
            != match request.action() {
                FractionalExposureActionV2::Wrap => request.input().destination_token_account,
                FractionalExposureActionV2::WholeUnwrap => request.input().source_token_account,
                _ => return Err(ClaimsSbfError::Instruction.into()),
            }
        || !token.executable
        || token.is_writable
        || token.is_signer
        || account(accounts, CLAIMS_PROGRAM)?.key != program_id
        || [root.key, actor.key, mint.key, holder.key, token.key]
            .iter()
            .enumerate()
            .any(|(left, key)| {
                [root.key, actor.key, mint.key, holder.key, token.key]
                    .iter()
                    .skip(left.saturating_add(1))
                    .any(|right| right == key)
            })
    {
        return Err(ClaimsSbfError::Accounts.into());
    }
    Ok(())
}

fn authenticate_terminal_tail_privileges(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: FractionalExposureRequestV2,
) -> Result<(), ProgramError> {
    for index in [
        FRACTIONAL_TERMINAL_TERMS_RAW_V3,
        FRACTIONAL_TERMINAL_TERMS_STAGING_V3,
        FRACTIONAL_TERMINAL_TOKEN_BEHAVIOR_RAW_V3,
        FRACTIONAL_TERMINAL_TOKEN_BEHAVIOR_STAGING_V3,
    ] {
        let value = account(accounts, index)?;
        if value.is_writable || value.is_signer || value.executable {
            return Err(ClaimsSbfError::Accounts.into());
        }
    }
    let root = account(accounts, FRACTIONAL_TERMINAL_ROOT_V3)?;
    let actor = account(accounts, FRACTIONAL_TERMINAL_ACTOR_V3)?;
    let mint = account(accounts, FRACTIONAL_TERMINAL_SHARD_MINT_V3)?;
    let source = account(accounts, FRACTIONAL_TERMINAL_SOURCE_TOKEN_V3)?;
    if !root.is_signer
        || !root.is_writable
        || root.executable
        || !actor.is_signer
        || actor.is_writable
        || actor.executable
        || actor.key.to_bytes() != request.input().owner
        || !mint.is_writable
        || mint.is_signer
        || mint.executable
        || !source.is_writable
        || source.is_signer
        || source.executable
        || source.key.to_bytes() != request.input().source_token_account
        || account(accounts, CLAIMS_PROGRAM)?.key != program_id
        || [root.key, actor.key, mint.key, source.key]
            .iter()
            .enumerate()
            .any(|(left, key)| {
                [root.key, actor.key, mint.key, source.key]
                    .iter()
                    .skip(left.saturating_add(1))
                    .any(|right| right == key)
            })
    {
        return Err(ClaimsSbfError::Accounts.into());
    }
    Ok(())
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| ClaimsSbfError::Accounts.into())
}
